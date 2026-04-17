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
    command_palette, confirm_dialog, container_actions, container_picker, context_picker, footer,
    header, namespace_bar, namespace_picker, resource_table, toast,
};

/// Fullscreen overlay modes that replace the entire view.
fn is_fullscreen_overlay(mode: &Mode) -> bool {
    matches!(
        mode,
        Mode::Help
            | Mode::NamespacePicker
            | Mode::ContextPicker
            | Mode::ContainerPicker(_)
            | Mode::ContainerActions { .. }
            | Mode::ConfirmAction(_)
            | Mode::SetImageInput
            | Mode::PortForwardInput
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
fn print_command(program: &str, args: &[String]) {
    let cmd_line = if args.is_empty() {
        program.to_string()
    } else {
        format!("{} {}", program, args.join(" "))
    };
    eprintln!("\x1b[2m→ {}\x1b[0m", cmd_line);
}

/// Ignore SIGINT in k9t so that Ctrl-C during a subprocess only kills the child,
/// not k9t itself. Returns the previous handler so it can be restored.
fn ignore_sigint() {
    unsafe {
        libc::signal(libc::SIGINT, libc::SIG_IGN);
    }
}

/// Restore default SIGINT handling after a subprocess finishes.
fn restore_sigint() {
    unsafe {
        libc::signal(libc::SIGINT, libc::SIG_DFL);
    }
}

/// Trait to add `pre_exec_restore_sigint()` to `Command`.
/// Sets SIG_DFL for SIGINT in the child before exec, undoing k9t's SIG_IGN
/// so the child still responds to Ctrl-C.
trait PreExecSigint: std::os::unix::process::CommandExt {
    fn pre_exec_restore_sigint(&mut self) -> &mut Self {
        unsafe {
            self.pre_exec(|| {
                libc::signal(libc::SIGINT, libc::SIG_DFL);
                Ok(())
            });
        }
        self
    }
}
impl PreExecSigint for std::process::Command {}

/// Suspend the TUI, run a kubectl subcommand (shell, logs, describe, yaml), then resume the TUI.
/// For shell exec commands, automatically retries fallback shells if exit code is 126
/// (shell not found in container).
/// Returns a re-initialized terminal.
fn run_subcommand(
    cmd: &ShellCommand,
) -> ratatui::Terminal<ratatui::prelude::CrosstermBackend<std::io::Stdout>> {
    // Restore terminal to normal mode so the subprocess can use it
    ratatui::restore();

    // Ignore SIGINT in k9t so Ctrl-C only kills the child process.
    // The child restores SIG_DFL via pre_exec so it still responds to Ctrl-C.
    ignore_sigint();

    // Run command directly — user can scroll back in terminal
    print_command(&cmd.program, &cmd.args);
    let exit_code = run_single_command(&cmd.program, &cmd.args);

    // For exec commands, if exit code is 126/127 (command not found in container),
    // try each fallback shell in order (sh → /bin/sh → /bin/bash)
    if exit_code == Some(126) || exit_code == Some(127) {
        for fallback in &cmd.fallback_commands {
            print_command(&fallback.program, &fallback.args);
            let fallback_code = run_single_command(&fallback.program, &fallback.args);
            // If the shell connected or user exited normally, stop retrying
            if fallback_code != Some(126) && fallback_code != Some(127) {
                break;
            }
        }
    }

    // Restore SIGINT handling now that the subprocess is done.
    restore_sigint();

    // Drain any leftover Ctrl-C key events that leaked from the subprocess.
    // With the SIG_IGN approach this is a belt-and-suspenders, but kept for safety.
    drain_ctrl_c_events();

    // Re-initialize the terminal for TUI rendering
    ratatui::init()
}

/// Run a single command and return its exit code (if it ran at all).
/// On failure to spawn, prints an error and waits for Enter.
///
/// The caller should call `ignore_sigint()` before calling this (and
/// `restore_sigint()` after) so that Ctrl-C only kills the child, not k9t.
/// We use `pre_exec` to restore SIG_DFL in the child so it still responds to Ctrl-C.
fn run_single_command(program: &str, args: &[String]) -> Option<i32> {
    let status = std::process::Command::new(program)
        .args(args)
        .pre_exec_restore_sigint()
        .status();

    match status {
        Ok(s) => {
            let code = s.code();
            if !s.success()
                && code != Some(126)
                && code != Some(127)
                && !matches!(code, Some(130) | Some(141))
            {
                // 126/127 = shell not found (retry with fallbacks)
                // 130 = Ctrl-C, 141 = SIGPIPE (normal exits, not errors)
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

/// RAII guard that restores the terminal when dropped, ensuring cleanup
/// even if the main loop exits via panic, early return, or normal quit.
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // Disable mouse capture before restoring the terminal.
        // ratatui::restore() handles raw mode and alternate screen, but
        // mouse capture was enabled separately and must be disabled separately.
        crossterm::execute!(std::io::stderr(), crossterm::event::DisableMouseCapture).unwrap_or(());
        ratatui::restore();
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Install a panic hook that restores the terminal (including disabling mouse
    // capture) before printing the panic. Without this, a panic leaves the
    // terminal in raw mode with mouse capture still active.
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        crossterm::execute!(std::io::stderr(), crossterm::event::DisableMouseCapture).unwrap_or(());
        ratatui::restore();
        default_panic(info);
    }));

    let cli = Cli::parse();

    let config = Config::load().unwrap_or_else(|e| {
        eprintln!("Warning: failed to load config: {e}");
        Config::default()
    });

    let client = create_client(cli.context.as_deref()).await.map_err(|e| {
        let msg = format!("{e}");
        if msg.contains("401") || msg.contains("Unauthorized") || msg.contains("unauthorized") {
            anyhow::anyhow!(
                "Unauthorized: Kubernetes API returned 401. Check your credentials and kubeconfig."
            )
        } else if msg.contains("403") || msg.contains("Forbidden") || msg.contains("forbidden") {
            anyhow::anyhow!("Forbidden: Kubernetes API returned 403. Check your RBAC permissions.")
        } else {
            anyhow::anyhow!("Cannot connect to Kubernetes: {e}. Check your kubeconfig.")
        }
    })?;

    let mut client = client;
    let mut reflector = PodReflector::start(client.clone())?;

    let mut terminal = ratatui::init();
    // Enable mouse capture for click and scroll support
    crossterm::execute!(std::io::stderr(), crossterm::event::EnableMouseCapture).unwrap_or(());

    // Guard restores the terminal when dropped — even on panic or early return.
    let _guard = TerminalGuard;
    let mut theme = Theme::auto();

    let mut app = App::with_commands(
        resolve_context_name(cli.context.as_deref()).await.ok(),
        config.commands.clone(),
        config.commands_builtin.clone(),
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
                        Event::Mouse(mouse) => {
                            app.update(AppEvent::Mouse(mouse));
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
                    let err_msg = format!("{e}");
                    let (msg, toast_type) =
                        if err_msg.contains("401") || err_msg.contains("Unauthorized") {
                            (
                                "Unauthorized: Check your Kubernetes credentials.".to_string(),
                                k9t_app::ToastType::Error,
                            )
                        } else if err_msg.contains("403") || err_msg.contains("Forbidden") {
                            (
                                "Forbidden: Check your RBAC permissions.".to_string(),
                                k9t_app::ToastType::Error,
                            )
                        } else {
                            match &action {
                                AsyncAction::KillPod { name, .. } => (
                                    format!("Failed to delete {}: {}", name, e),
                                    k9t_app::ToastType::Error,
                                ),
                                AsyncAction::RestartDeployment { name, .. } => (
                                    format!("Failed to restart {}: {}", name, e),
                                    k9t_app::ToastType::Error,
                                ),
                            }
                        };
                    app.show_toast(msg, toast_type, 10);
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
                    let err_msg = format!("{e}");
                    let msg = if err_msg.contains("401") || err_msg.contains("Unauthorized") {
                        "Unauthorized: Check your Kubernetes credentials.".to_string()
                    } else if err_msg.contains("403") || err_msg.contains("Forbidden") {
                        "Forbidden: Check your RBAC permissions.".to_string()
                    } else {
                        format!("Failed to switch context: {e}")
                    };
                    app.show_toast(msg, k9t_app::ToastType::Error, 10);
                }
            }
        }

        terminal.draw(|frame: &mut ratatui::Frame| {
            let area = frame.area();

            if is_terminal_too_small(area) {
                use ratatui::widgets::Paragraph;
                let msg = Paragraph::new("Terminal too small — k9t requires at least 80x12")
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
                Mode::ContainerActions { .. } => "ACTIONS",
                Mode::ContextPicker => "CONTEXTS",
                Mode::SetImageInput => "SET IMAGE",
                Mode::PortForwardInput => "PORT FORWARD",
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
                let title = app.table_title();
                resource_table::render_pod_table(
                    frame,
                    layout.table,
                    &rows,
                    app.selected_index,
                    &title,
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
                            ("f", "port-forward"),
                            ("D", "debug"),
                            ("i", "image"),
                            (",", "sort"),
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
                    let selected = app.effective_selected_namespaces();
                    namespace_picker::render_namespace_picker(
                        frame,
                        frame.area(),
                        &app.active_namespaces(),
                        selected,
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
                        k9t_app::ContainerPickerIntent::PortForward => "Port Forward",
                        k9t_app::ContainerPickerIntent::Debug => "Debug",
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
                Mode::ContainerActions { .. } => {
                    let (pod_name, container_name) =
                        if let Some(row) = app.table_rows().get(app.selected_index) {
                            match row {
                                k9t_app::TableRow::Container {
                                    pod_index,
                                    container,
                                    ..
                                } => {
                                    let pod_name = app
                                        .pods
                                        .get(*pod_index)
                                        .map(|p| p.name.clone())
                                        .unwrap_or_default();
                                    (pod_name, container.name.clone())
                                }
                                _ => (String::new(), String::new()),
                            }
                        } else {
                            (String::new(), String::new())
                        };
                    let (query, index) = match &app.mode {
                        k9t_app::Mode::ContainerActions { query, index } => (query.clone(), *index),
                        _ => (String::new(), 0),
                    };
                    let filtered = app.filtered_container_actions(&query);
                    let clamped_index = index.min(filtered.len().saturating_sub(1));
                    container_actions::render_container_actions(
                        frame,
                        frame.area(),
                        &filtered,
                        clamped_index,
                        &query,
                        &pod_name,
                        &container_name,
                        &theme,
                    );
                }
                Mode::SetImageInput => {
                    let container = app.set_image_container.as_str();
                    let pod = app.set_image_pod.as_str();
                    let ns = app.set_image_namespace.as_str();
                    let label = format!("{ns}/{pod}/{container}");
                    let input = app.set_image_buffer.as_str();
                    let placeholder = if input.is_empty() { "image:tag" } else { "" };
                    confirm_dialog::render_input_dialog(
                        frame,
                        area,
                        "Set Image",
                        &label,
                        input,
                        placeholder,
                        "[Enter]apply  [Esc]cancel  [Ctrl+U]clear",
                        &theme,
                    );
                }
                Mode::PortForwardInput => {
                    let pod = app.port_forward_pod.as_str();
                    let ns = app.port_forward_namespace.as_str();
                    let container_info = app
                        .port_forward_container
                        .as_deref()
                        .map(|c| format!("/{c}"))
                        .unwrap_or_default();
                    let label = format!("Port forward {ns}/{pod}{container_info}:");
                    let input = app.port_forward_buffer.as_str();
                    let placeholder = if input.is_empty() {
                        app.port_forward_suggestion()
                    } else {
                        String::new()
                    };
                    confirm_dialog::render_input_dialog(
                        frame,
                        area,
                        "Port Forward",
                        &label,
                        input,
                        &placeholder,
                        "[Enter]apply  [Esc]cancel  [Ctrl+U]clear",
                        &theme,
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

                    // ── Left column ──
                    let left_lines: Vec<ratatui::text::Line> = vec![
                        ratatui::text::Line::from(ratatui::text::Span::styled(
                            " k9t — Kubernetes Terminal UI",
                            accent,
                        )),
                        ratatui::text::Line::from(ratatui::text::Span::styled(
                            "   (k9s-compatible keybindings)",
                            dim,
                        )),
                        ratatui::text::Line::from(""),
                        // Navigation
                        ratatui::text::Line::from(ratatui::text::Span::styled(
                            " Navigation",
                            title,
                        )),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   j/k ↑/↓      ", emphasis),
                            ratatui::text::Span::styled("Move selection", dim),
                        ]),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   Enter         ", emphasis),
                            ratatui::text::Span::styled("Expand/collapse", dim),
                        ]),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   g/G Home/End  ", emphasis),
                            ratatui::text::Span::styled("Top/bottom", dim),
                        ]),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   Esc           ", emphasis),
                            ratatui::text::Span::styled("Back / close", dim),
                        ]),
                        ratatui::text::Line::from(""),
                        // Actions
                        ratatui::text::Line::from(ratatui::text::Span::styled(" Actions", title)),
                        ratatui::text::Line::from(ratatui::text::Span::styled(
                            "   (on container: targets that container)",
                            dim,
                        )),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   l             ", emphasis),
                            ratatui::text::Span::styled("Logs (kubectl logs)", dim),
                        ]),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   p             ", emphasis),
                            ratatui::text::Span::styled("Previous logs", dim),
                        ]),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   s             ", emphasis),
                            ratatui::text::Span::styled("Shell (kubectl exec)", dim),
                        ]),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   d             ", emphasis),
                            ratatui::text::Span::styled("Describe pod", dim),
                        ]),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   y             ", emphasis),
                            ratatui::text::Span::styled("View YAML", dim),
                        ]),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   i             ", emphasis),
                            ratatui::text::Span::styled("Set container image", dim),
                        ]),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   f             ", emphasis),
                            ratatui::text::Span::styled("Port forward", dim),
                        ]),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   D             ", emphasis),
                            ratatui::text::Span::styled("Debug pod (kubectl debug)", dim),
                        ]),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled(
                                "   K             ",
                                theme
                                    .status_warning()
                                    .add_modifier(ratatui::style::Modifier::BOLD),
                            ),
                            ratatui::text::Span::styled("Kill pod", dim),
                        ]),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled(
                                "   R             ",
                                theme
                                    .status_warning()
                                    .add_modifier(ratatui::style::Modifier::BOLD),
                            ),
                            ratatui::text::Span::styled("Restart deployment", dim),
                        ]),
                    ];

                    // ── Right column ──
                    let mut right_lines: Vec<ratatui::text::Line> = vec![
                        ratatui::text::Line::from(""),
                        ratatui::text::Line::from(""),
                        // Search / Filter / Sort
                        ratatui::text::Line::from(ratatui::text::Span::styled(
                            " Search / Sort",
                            title,
                        )),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   /             ", emphasis),
                            ratatui::text::Span::styled("Filter pods", dim),
                        ]),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   ,             ", emphasis),
                            ratatui::text::Span::styled("Cycle sort order", dim),
                        ]),
                        ratatui::text::Line::from(""),
                        // Command Mode
                        ratatui::text::Line::from(ratatui::text::Span::styled(
                            " Command Mode  (press :)",
                            title,
                        )),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   :q  :quit     ", cmd_style),
                            ratatui::text::Span::styled("Quit k9t", dim),
                        ]),
                        ratatui::text::Line::from(""),
                        // UI
                        ratatui::text::Line::from(ratatui::text::Span::styled(" UI", title)),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   n             ", emphasis),
                            ratatui::text::Span::styled("Namespace picker", dim),
                        ]),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   x             ", emphasis),
                            ratatui::text::Span::styled("Context picker", dim),
                        ]),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   Shift+T       ", emphasis),
                            ratatui::text::Span::styled("Cycle theme", dim),
                        ]),
                        ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled("   Ctrl-C        ", emphasis),
                            ratatui::text::Span::styled("Quit k9t", dim),
                        ]),
                    ];

                    // ── Custom Commands (from config) ──
                    if !app.custom_commands.is_empty() {
                        right_lines.push(ratatui::text::Line::from(""));
                        right_lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                            " Custom Commands  (~/.config/k9t.yaml)",
                            title,
                        )));
                        for cc in &app.custom_commands {
                            let desc = cc.description.as_deref().unwrap_or(&cc.command);
                            let desc_display = if desc.len() > 38 {
                                format!("{}…", &desc[..35])
                            } else {
                                desc.to_string()
                            };
                            right_lines.push(ratatui::text::Line::from(vec![
                                ratatui::text::Span::styled(
                                    format!("   :{:<10}", cc.name),
                                    cmd_style,
                                ),
                                ratatui::text::Span::styled(desc_display, dim),
                            ]));
                        }
                    }

                    right_lines.push(ratatui::text::Line::from(""));
                    right_lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                        " Press ? or Esc to close",
                        dim,
                    )));

                    // Render two columns
                    let [left_area, right_area] = ratatui::layout::Layout::horizontal([
                        ratatui::layout::Constraint::Percentage(50),
                        ratatui::layout::Constraint::Percentage(50),
                    ])
                    .areas(frame.area());

                    let left_widget =
                        ratatui::widgets::Paragraph::new(left_lines).style(theme.bg_surface());
                    let right_widget =
                        ratatui::widgets::Paragraph::new(right_lines).style(theme.bg_surface());
                    frame.render_widget(left_widget, left_area);
                    frame.render_widget(right_widget, right_area);
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

    // Drop the EventStream explicitly so crossterm's background reader thread
    // stops before we restore the terminal. This prevents the reader thread
    // from interfering with the terminal state after restore.
    drop(events);

    // TerminalGuard drops here and calls ratatui::restore().
    Ok(())
}
