use std::io::Write;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use crossterm::event::{Event, EventStream};
use futures::StreamExt;

use k9t_app::{App, AppEvent, AsyncAction, Config, ConfirmContext, Mode, ShellCommand};
use k9t_core::{
    PodReflector, create_client, delete_pod, discover_contexts, discover_namespaces,
    resolve_context_name, restart_deployment,
};
use k9t_ui::layout::AppLayout;
use k9t_ui::layout::is_terminal_too_small;
use k9t_ui::theme::Theme;
use k9t_ui::widgets::{
    command_palette, confirm_dialog, container_picker, context_picker, footer, header,
    namespace_bar, namespace_picker, resource_table, toast,
};

/// Fullscreen overlay modes that replace the entire view.
fn is_fullscreen_overlay(mode: &Mode) -> bool {
    matches!(
        mode,
        Mode::Help
            | Mode::NamespacePicker
            | Mode::ContextPicker
            | Mode::ContainerPicker(_)
            | Mode::ConfirmAction(_)
            | Mode::SetImageInput
    )
}

#[derive(Parser)]
#[command(name = "k9t", about = "Kubernetes terminal UI")]
struct Cli {
    #[arg(short, long)]
    context: Option<String>,
    #[arg(short, long)]
    namespace: Option<String>,
    #[arg(short = 'A', long)]
    all_namespaces: bool,
    #[arg(long)]
    kubeconfig: Option<String>,
    #[arg(long = "regex-namespace-pods", value_name = "NAMESPACE/POD_PATTERN")]
    regex_namespace_pods: Vec<String>,
}

/// Print the command being run so the user can see it and copy-paste it if needed.
/// For piped commands, shows the full pipeline.
fn print_command(program: &str, args: &[String]) {
    let cmd_line = if args.is_empty() {
        program.to_string()
    } else {
        format!("{} {}", program, args.join(" "))
    };
    eprintln!("\x1b[2m→ {}\x1b[0m", cmd_line);
}

/// Print a full pipeline command (e.g. kubectl ... | jq ... | less).
fn print_pipeline(parts: &[&str]) {
    eprintln!("\x1b[2m→ {}\x1b[0m", parts.join(" | "));
}

/// Suspend the TUI, run a kubectl subcommand (shell, edit, yaml view), then resume the TUI.
/// For shell exec commands, automatically retries fallback shells if exit code is 126
/// (shell not found in container).
/// For non-interactive commands (describe, yaml, delete), pipes output through `less -RFX`
/// so the user can scroll and the output doesn't flash away.
/// Returns a re-initialized terminal.
fn run_subcommand(
    cmd: &ShellCommand,
) -> ratatui::Terminal<ratatui::prelude::CrosstermBackend<std::io::Stdout>> {
    // Restore terminal to normal mode so the subprocess can use it
    ratatui::restore();

    if cmd.needs_pause {
        // Non-interactive command: pipe through less -RFX
        run_paged_command(&cmd.program, &cmd.args, cmd.jq_filter.as_deref());
    } else {
        // Interactive command (exec, logs -f, edit): run directly
        print_command(&cmd.program, &cmd.args);
        let exit_code = run_single_command(&cmd.program, &cmd.args);

        // For exec commands, if exit code is 126 (command not found in container),
        // try each fallback shell in order (k9s behavior: try /bin/bash → /bin/sh → sh)
        if exit_code == Some(126) {
            for fallback in &cmd.fallback_commands {
                print_command(&fallback.program, &fallback.args);
                let fallback_code = run_single_command(&fallback.program, &fallback.args);
                // If the shell connected or user exited normally, stop retrying
                if fallback_code != Some(126) {
                    break;
                }
            }
        }
    }

    // Drain any leftover Ctrl-C key events that leaked from the subprocess.
    // Without this, the Ctrl-C that killed `kubectl logs -f` would also quit k9t.
    drain_ctrl_c_events();

    // Re-initialize the terminal for TUI rendering
    ratatui::init()
}

/// Check if an external command exists on PATH.
fn command_exists(name: &str) -> bool {
    std::process::Command::new(name)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

/// Run a command and pipe its output through `less -RFX` for paging.
/// If `jq_filter` is Some, inserts `jq -Rr '<filter>'` between the command and less
/// to pretty-print JSON log lines while passing plain text through.
fn run_paged_command(program: &str, args: &[String], jq_filter: Option<&str>) {
    let less_available = command_exists("less");
    let use_jq = jq_filter.is_some() && command_exists("jq");

    // Print the full pipeline so the user can see and copy-paste it
    {
        let kubectl_cmd = format!("{} {}", program, args.join(" "));
        if use_jq {
            if let Some(filter) = jq_filter {
                print_pipeline(&[
                    &kubectl_cmd,
                    &format!("jq --unbuffered -Rr '{}'", filter),
                    "less -RFX",
                ]);
            }
        } else {
            print_command(program, args);
        }
        // Flush so the user sees it before less takes over the terminal
        let _ = std::io::stderr().flush();
    }

    if less_available {
        // Spawn kubectl with piped stdout
        let kubectl = match std::process::Command::new(program)
            .args(args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                eprintln!("\nk9t: failed to run '{}': {}", program, e);
                eprintln!("Press Enter to return to k9t...");
                let _ = std::io::stdin().read_line(&mut String::new());
                return;
            }
        };

        let kubectl_stdout = kubectl.stdout.unwrap();

        // Pipeline: kubectl → [jq] → less
        if let Some(filter) = jq_filter.filter(|_| use_jq) {
            // kubectl → jq → less
            let jq = std::process::Command::new("jq")
                .arg("--unbuffered")
                .arg("-Rr")
                .arg(filter)
                .stdin(kubectl_stdout)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn();

            match jq {
                Ok(jq_proc) => {
                    let less = std::process::Command::new("less")
                        .arg("-RFX")
                        .stdin(jq_proc.stdout.unwrap())
                        .spawn();
                    match less {
                        Ok(mut less_proc) => {
                            let _ = less_proc.wait();
                        }
                        Err(e) => {
                            eprintln!("\nk9t: failed to run 'less': {}", e);
                            eprintln!("Press Enter to return to k9t...");
                            let _ = std::io::stdin().read_line(&mut String::new());
                        }
                    }
                }
                Err(e) => {
                    eprintln!("\nk9t: failed to run 'jq': {}", e);
                    eprintln!("Press Enter to return to k9t...");
                    let _ = std::io::stdin().read_line(&mut String::new());
                }
            }
        } else {
            // kubectl → less (no jq)
            let less = std::process::Command::new("less")
                .arg("-RFX")
                .stdin(kubectl_stdout)
                .spawn();

            match less {
                Ok(mut less_proc) => {
                    let _ = less_proc.wait();
                }
                Err(e) => {
                    eprintln!("\nk9t: failed to run 'less': {}", e);
                    eprintln!("Press Enter to return to k9t...");
                    let _ = std::io::stdin().read_line(&mut String::new());
                }
            }
        }
    } else {
        // No less available — run command and pause after
        let status = std::process::Command::new(program).args(args).status();

        if let Ok(s) = status
            && !s.success()
        {
            eprintln!(
                "\nk9t: command exited with code: {}",
                s.code().unwrap_or(-1)
            );
        }
        eprintln!("\n--- Press Enter to return to k9t ---");
        let _ = std::io::stdin().read_line(&mut String::new());
    }
}

/// Run a single command and return its exit code (if it ran at all).
/// On failure to spawn, prints an error and waits for Enter.
fn run_single_command(program: &str, args: &[String]) -> Option<i32> {
    let status = std::process::Command::new(program).args(args).status();

    match status {
        Ok(s) => {
            let code = s.code();
            if !s.success() && code != Some(126) && code != Some(130) {
                // 126 = shell not found (retry with fallbacks)
                // 130 = Ctrl+C exit (normal for interactive commands like logs -f)
                eprintln!("\nk9t: command exited with code: {}", code.unwrap_or(-1));
                eprintln!("Press Enter to return to k9t...");
                let _ = std::io::stdin().read_line(&mut String::new());
            }
            code
        }
        Err(e) => {
            eprintln!("\nk9t: failed to run '{}': {}", program, e);
            eprintln!("Press Enter to return to k9t...");
            let _ = std::io::stdin().read_line(&mut String::new());
            None
        }
    }
}

/// After running a subcommand (especially interactive ones like `kubectl logs -f`),
/// the user may have pressed Ctrl-C to exit the subprocess. That SIGINT can leak
/// through as a crossterm key event that would quit k9t on the next loop iteration.
/// We drain any pending Ctrl-C events to prevent this.
fn drain_ctrl_c_events() {
    // Poll for pending events with zero timeout to drain any leftover Ctrl-C
    while crossterm::event::poll(std::time::Duration::ZERO).unwrap_or(false) {
        let _ = crossterm::event::read();
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let config = Config::load().unwrap_or_else(|e| {
        eprintln!("Warning: failed to load config: {e}");
        Config::default()
    });

    let client = create_client(cli.context.as_deref()).await.map_err(|e| {
        anyhow::anyhow!("Cannot connect to Kubernetes: {e}. Check your kubeconfig.")
    })?;

    let mut client = client;
    let mut reflector = PodReflector::start(client.clone())?;

    let mut terminal = ratatui::init();
    let mut theme = Theme::auto();

    let mut app = App::with_commands(
        resolve_context_name(cli.context.as_deref()).await.ok(),
        config.commands.clone(),
    );
    if let Some(ns) = cli.namespace.as_deref().or(config.namespace.as_deref()) {
        app.namespace_filter = ns.to_string();
    }
    if cli.all_namespaces {
        app.namespace_filter = "*".to_string();
    }
    // CLI filters override config filters (both are OR'd together within the filter list)
    for pattern in &cli.regex_namespace_pods {
        if let Err(e) = app.add_namespace_pod_filter(pattern) {
            eprintln!("Warning: invalid filter '{}': {e}", pattern);
        }
    }
    // Config filters
    for pattern in &config.filters {
        if let Err(e) = app.add_namespace_pod_filter(pattern) {
            eprintln!("Warning: invalid config filter '{}': {e}", pattern);
        }
    }

    let available_namespaces = discover_namespaces(&client).await.unwrap_or_default();
    app.set_available_namespaces(available_namespaces);

    let available_contexts = discover_contexts().unwrap_or_default();
    app.set_available_contexts(available_contexts);

    let mut events = EventStream::new();
    let refresh_rate = Duration::from_millis(config.refresh_rate_ms);
    let mut tick = tokio::time::interval(refresh_rate);

    loop {
        tokio::select! {
            maybe_event = events.next() => {
                if let Some(Ok(event)) = maybe_event {
                    match event {
                        Event::Key(key) => {
                            if key.code == crossterm::event::KeyCode::Char('c')
                                && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)
                            {
                                break;
                            }
                            if app.update(AppEvent::Key(key)) {
                                break;
                            }
                        }
                        Event::Resize(w, h) => {
                            app.update(AppEvent::Resize(w, h));
                        }
                        _ => {}
                    }
                }
            }
            _ = tick.tick() => {
                let pods = reflector.store();
                app.set_pods(pods);
                app.update(AppEvent::Tick);
            }
        }

        // Handle pending kubectl subcommands (shell, edit, yaml) — suspend TUI, run, resume
        if let Some(shell_cmd) = app.pending_shell.take() {
            terminal = run_subcommand(&shell_cmd);
        }

        // Handle pending async Kubernetes actions (kill pod, restart deployment)
        if let Some(action) = app.pending_async_action.take() {
            let client_clone = client.clone();
            let result = match &action {
                AsyncAction::KillPod { namespace, name } => {
                    delete_pod(&client_clone, namespace, name).await
                }
                AsyncAction::RestartDeployment { namespace, name } => {
                    restart_deployment(&client_clone, namespace, name).await
                }
            };
            match result {
                Ok(()) => {
                    let msg = match &action {
                        AsyncAction::KillPod { name, .. } => format!("Pod {} deleted", name),
                        AsyncAction::RestartDeployment { name, .. } => {
                            format!("Restart triggered for {}", name)
                        }
                    };
                    app.show_toast(msg, k9t_app::ToastType::Success, 6);
                }
                Err(e) => {
                    let msg = match &action {
                        AsyncAction::KillPod { name, .. } => {
                            format!("Failed to delete {}: {}", name, e)
                        }
                        AsyncAction::RestartDeployment { name, .. } => {
                            format!("Failed to restart {}: {}", name, e)
                        }
                    };
                    app.show_toast(msg, k9t_app::ToastType::Error, 10);
                }
            }
        }

        // Handle pending kube context switches — re-create client, refresh namespaces, restart reflector
        if let Some(new_context) = app.pending_context_switch.take() {
            match create_client(Some(&new_context)).await {
                Ok(new_client) => {
                    client = new_client.clone();
                    reflector = PodReflector::start(client.clone())?;
                    app.apply_context_switch(new_context.clone());
                    let available_namespaces =
                        discover_namespaces(&client).await.unwrap_or_default();
                    app.set_available_namespaces(available_namespaces);
                    app.show_toast(
                        format!("Switched to context {}", new_context),
                        k9t_app::ToastType::Success,
                        6,
                    );
                }
                Err(e) => {
                    app.show_toast(
                        format!("Failed to switch context: {e}"),
                        k9t_app::ToastType::Error,
                        10,
                    );
                }
            }
        }

        terminal.draw(|frame: &mut ratatui::Frame| {
            let area = frame.area();

            if is_terminal_too_small(area) {
                use ratatui::widgets::Paragraph;
                let msg = Paragraph::new("Terminal too small — k9t requires at least 80x24")
                    .style(ratatui::style::Style::default().fg(ratatui::style::Color::Red));
                frame.render_widget(msg, area);
                return;
            }

            let all_themes = Theme::all_themes();
            if app.theme_index < all_themes.len() {
                theme = all_themes[app.theme_index].clone();
            }

            // Render base view (always visible unless fullscreen overlay)
            let layout = AppLayout::new(area);
            let mode_name = match &app.mode {
                Mode::Normal => "NORMAL",
                Mode::CommandPalette { .. } => "COMMAND",
                Mode::ConfirmAction(_) => "CONFIRM",
                Mode::Search(_) => "SEARCH",
                Mode::Help => "HELP",
                Mode::NamespacePicker => "NAMESPACES",
                Mode::ContainerPicker(_) => "CONTAINERS",
                Mode::ContextPicker => "CONTEXTS",
                Mode::SetImageInput => "SET IMAGE",
            };

            let ns_display = if app.namespace_pod_filters.is_empty() {
                app.namespace_filter.as_str()
            } else {
                "(filtered)"
            };

            // Fullscreen overlays: clear entire screen, then render overlay
            if is_fullscreen_overlay(&app.mode) {
                frame.render_widget(ratatui::widgets::Clear, area);
            }

            // Always render header + footer for context (even in overlays)
            header::render_header(
                frame,
                layout.header,
                app.context_name.as_deref().unwrap_or("unknown"),
                ns_display,
                &theme,
            );

            // Skip namespace bar + resource table when a fullscreen overlay is active
            if !is_fullscreen_overlay(&app.mode) {
                namespace_bar::render_namespace_bar(
                    frame,
                    layout.namespace_bar,
                    &app.selected_namespaces,
                    &app.active_namespaces(),
                    &theme,
                );
                let rows = app.table_rows();
                resource_table::render_pod_table(
                    frame,
                    layout.table,
                    &rows,
                    app.selected_index,
                    &theme,
                );
            }

            // Context-sensitive footer / bottom bars
            match &app.mode {
                Mode::Search(input) => {
                    let search_spans = ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(
                            "/",
                            theme
                                .accent_primary()
                                .add_modifier(ratatui::style::Modifier::BOLD),
                        ),
                        ratatui::text::Span::styled(input.to_string(), theme.fg_default()),
                        ratatui::text::Span::styled("█", theme.accent_primary()),
                    ]);
                    frame.render_widget(
                        ratatui::widgets::Paragraph::new(search_spans).style(theme.bg_overlay()),
                        layout.footer,
                    );
                }
                Mode::CommandPalette { query, index } => {
                    let filtered = app.filtered_command_items();
                    let items: Vec<command_palette::CommandItem> = filtered
                        .iter()
                        .map(|item| command_palette::CommandItem {
                            name: item.name.clone(),
                            description: item.description.clone(),
                            is_custom: item.is_custom,
                        })
                        .collect();
                    command_palette::render_command_palette(
                        frame, area, query, &items, *index, &theme,
                    );
                }
                _ => {
                    // Show contextual hints based on mode
                    let context_hints: &[(&str, &str)] = match &app.mode {
                        Mode::ContainerPicker(_) => {
                            &[("Enter", "select"), ("j/k", "nav"), ("Esc", "cancel")]
                        }
                        Mode::ContextPicker => {
                            &[("Enter", "select"), ("j/k", "nav"), ("Esc", "cancel")]
                        }
                        Mode::SetImageInput => {
                            &[("Enter", "apply"), ("Esc", "cancel"), ("Ctrl+U", "clear")]
                        }
                        Mode::Help => &[("Esc/?", "close")],
                        _ => &[
                            ("j/k", "nav"),
                            ("Enter", "expand"),
                            ("l", "logs"),
                            ("s", "shell"),
                            ("i", "image"),
                            ("K", "kill"),
                            ("R", "restart"),
                            ("?", "help"),
                        ],
                    };
                    footer::render_footer(frame, layout.footer, mode_name, context_hints, &theme);
                }
            }

            // Overlays (rendered on top of base view)
            match &app.mode {
                Mode::ConfirmAction(ctx) => {
                    let (title, message, resource) = match ctx {
                        ConfirmContext::KillPod { namespace, name } => (
                            "Confirm Kill Pod",
                            "Delete pod",
                            &format!("{}/{}", namespace, name) as &str,
                        ),
                        ConfirmContext::RestartDeployment { namespace, name } => (
                            "Confirm Restart",
                            "Restart deployment for pod",
                            &format!("{}/{}", namespace, name) as &str,
                        ),
                    };
                    confirm_dialog::render_confirm_dialog(
                        frame,
                        frame.area(),
                        title,
                        message,
                        resource,
                        &theme,
                    );
                }
                Mode::NamespacePicker => {
                    namespace_picker::render_namespace_picker(
                        frame,
                        frame.area(),
                        &app.active_namespaces(),
                        &app.selected_namespaces,
                        app.namespace_picker_index,
                        &app.namespace_picker_search,
                        &theme,
                    );
                }
                Mode::ContextPicker => {
                    context_picker::render_context_picker(
                        frame,
                        frame.area(),
                        &app.available_contexts,
                        app.context_name.as_deref(),
                        app.context_picker_index,
                        &app.context_picker_search,
                        &theme,
                    );
                }
                Mode::ContainerPicker(intent) => {
                    let pod_name = app
                        .selected_pod_cloned()
                        .map(|p| p.name)
                        .unwrap_or_else(|| "unknown".to_string());
                    let title_suffix = match intent {
                        k9t_app::ContainerPickerIntent::Shell => "Shell",
                        k9t_app::ContainerPickerIntent::Logs(_) => "Logs",
                        k9t_app::ContainerPickerIntent::SetImage => "Set Image",
                    };
                    container_picker::render_container_picker(
                        frame,
                        frame.area(),
                        &app.container_choices,
                        app.container_picker_index,
                        &pod_name,
                        title_suffix,
                        &theme,
                    );
                }
                Mode::SetImageInput => {
                    let container = app.set_image_container.as_str();
                    let pod = app.set_image_pod.as_str();
                    let ns = app.set_image_namespace.as_str();
                    let input = app.set_image_buffer.as_str();
                    let prompt_label = format!(" Set image for {ns}/{pod}/{container}: ");
                    let mut spans = vec![
                        ratatui::text::Span::styled(prompt_label, theme.fg_muted()),
                        ratatui::text::Span::styled(input.to_string(), theme.fg_default()),
                        ratatui::text::Span::styled("█", theme.accent_primary()),
                    ];
                    if input.is_empty() {
                        spans.push(ratatui::text::Span::styled(
                            " <image:tag>",
                            theme.fg_muted(),
                        ));
                    }
                    let prompt = ratatui::widgets::Paragraph::new(ratatui::text::Line::from(spans))
                        .style(theme.bg_surface());
                    frame.render_widget(ratatui::widgets::Clear, area);
                    frame.render_widget(prompt, area);

                    let hint = " [Enter]apply  [Esc]cancel  [Ctrl+U]clear";
                    let hint_line = ratatui::text::Line::from(ratatui::text::Span::styled(
                        hint,
                        theme.fg_muted(),
                    ));
                    let hint_area = ratatui::layout::Rect::new(
                        area.x,
                        area.y + area.height.saturating_sub(2),
                        area.width.min(60),
                        1,
                    );
                    frame.render_widget(
                        ratatui::widgets::Paragraph::new(hint_line).style(theme.bg_surface()),
                        hint_area,
                    );
                }
                Mode::Help => {
                    let dim = theme.fg_muted();
                    let emphasis = theme.fg_emphasis();
                    let title = theme.title_style();
                    let accent = theme
                        .accent_primary()
                        .add_modifier(ratatui::style::Modifier::BOLD);
                    let cmd_style = theme.status_success();

                    // Build help lines dynamically to include custom commands
                    let mut help_lines: Vec<ratatui::text::Line> = vec![
                        ratatui::text::Line::from(""),
                        ratatui::text::Line::from(ratatui::text::Span::styled(
                            " k9t — Kubernetes Terminal UI",
                            accent,
                        )),
                        ratatui::text::Line::from(ratatui::text::Span::styled(
                            "   (k9s-compatible keybindings)",
                            dim,
                        )),
                        ratatui::text::Line::from(""),
                        // ── Navigation ──
                        ratatui::text::Line::from(ratatui::text::Span::styled(
                            " Navigation",
                            title,
                        )),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   j/k  ↑/↓     ", emphasis),
                            ratatui::text::Span::styled("Move selection", dim),
                        ]),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   Enter         ", emphasis),
                            ratatui::text::Span::styled("Expand/collapse pod containers", dim),
                        ]),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   g/G  Home/End ", emphasis),
                            ratatui::text::Span::styled("Jump to top/bottom", dim),
                        ]),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   Esc           ", emphasis),
                            ratatui::text::Span::styled("Go back / close overlay", dim),
                        ]),
                        ratatui::text::Line::from(""),
                        // ── Actions ──
                        ratatui::text::Line::from(ratatui::text::Span::styled(" Actions", title)),
                        ratatui::text::Line::from(ratatui::text::Span::styled(
                            "   (on container row: targets that container)",
                            dim,
                        )),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   l             ", emphasis),
                            ratatui::text::Span::styled("View logs (kubectl logs -f)", dim),
                        ]),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   p             ", emphasis),
                            ratatui::text::Span::styled("View previous logs", dim),
                        ]),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   s             ", emphasis),
                            ratatui::text::Span::styled("Shell into pod (kubectl exec)", dim),
                        ]),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   d             ", emphasis),
                            ratatui::text::Span::styled("Describe pod (kubectl describe)", dim),
                        ]),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   y             ", emphasis),
                            ratatui::text::Span::styled("View YAML (kubectl get -o yaml)", dim),
                        ]),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   i             ", emphasis),
                            ratatui::text::Span::styled(
                                "Set container image (kubectl set image)",
                                dim,
                            ),
                        ]),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled(
                                "   K             ",
                                theme
                                    .status_error()
                                    .add_modifier(ratatui::style::Modifier::BOLD),
                            ),
                            ratatui::text::Span::styled("Kill pod (with confirmation)", dim),
                        ]),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled(
                                "   R             ",
                                theme
                                    .status_warning()
                                    .add_modifier(ratatui::style::Modifier::BOLD),
                            ),
                            ratatui::text::Span::styled(
                                "Restart deployment (with confirmation)",
                                dim,
                            ),
                        ]),
                        ratatui::text::Line::from(""),
                        // ── Search / Filter ──
                        ratatui::text::Line::from(ratatui::text::Span::styled(
                            " Search / Filter",
                            title,
                        )),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   /             ", emphasis),
                            ratatui::text::Span::styled("Start search / filter", dim),
                        ]),
                        ratatui::text::Line::from(""),
                        // ── Command Mode ──
                        ratatui::text::Line::from(ratatui::text::Span::styled(
                            " Command Mode   (press : to enter)",
                            title,
                        )),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   :q  :quit      ", cmd_style),
                            ratatui::text::Span::styled("Quit k9t", dim),
                        ]),
                    ];

                    // ── Custom Commands (from config) ──
                    if !app.custom_commands.is_empty() {
                        help_lines.push(ratatui::text::Line::from(""));
                        help_lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                            " Custom Commands   (from ~/.config/k9t.json)",
                            title,
                        )));
                        for cc in &app.custom_commands {
                            let desc = cc.description.as_deref().unwrap_or(&cc.command);
                            // Truncate long descriptions for display
                            let desc_display = if desc.len() > 50 {
                                format!("{}…", &desc[..47])
                            } else {
                                desc.to_string()
                            };
                            help_lines.push(ratatui::text::Line::from(vec![
                                ratatui::text::Span::styled(
                                    format!("   :{:<14}", cc.name),
                                    cmd_style,
                                ),
                                ratatui::text::Span::styled(desc_display, dim),
                            ]));
                        }
                    }

                    help_lines.push(ratatui::text::Line::from(""));
                    // ── UI ──
                    help_lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                        " UI", title,
                    )));
                    help_lines.push(ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled("   n             ", emphasis),
                        ratatui::text::Span::styled("Open namespace picker", dim),
                    ]));
                    help_lines.push(ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled("   x             ", emphasis),
                        ratatui::text::Span::styled("Open context picker", dim),
                    ]));
                    help_lines.push(ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled("   Shift+T       ", emphasis),
                        ratatui::text::Span::styled("Cycle color theme", dim),
                    ]));
                    help_lines.push(ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled("   Ctrl-C        ", emphasis),
                        ratatui::text::Span::styled("Quit k9t", dim),
                    ]));
                    help_lines.push(ratatui::text::Line::from(""));
                    help_lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                        " Press ? or Esc to close this help",
                        dim,
                    )));

                    let help_widget =
                        ratatui::widgets::Paragraph::new(help_lines).style(theme.bg_surface());
                    frame.render_widget(help_widget, frame.area());
                }
                _ => {}
            }

            // Toast overlay (always on top)
            if let Some(ref msg) = app.toast_message {
                let toast_type = match app.toast_type {
                    k9t_app::ToastType::Info => toast::ToastType::Info,
                    k9t_app::ToastType::Success => toast::ToastType::Success,
                    k9t_app::ToastType::Warning => toast::ToastType::Warning,
                    k9t_app::ToastType::Error => toast::ToastType::Error,
                };
                let toast_area = ratatui::layout::Rect::new(
                    area.x,
                    area.y + area.height.saturating_sub(2),
                    area.width.min(60),
                    1,
                );
                toast::render_toast(frame, toast_area, msg, &toast_type, &theme);
            }
        })?;
    }

    ratatui::restore();
    Ok(())
}
