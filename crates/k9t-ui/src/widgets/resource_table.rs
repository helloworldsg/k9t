use ratatui::{
    Frame,
    layout::Constraint,
    style::Style,
    text::Span,
    widgets::{Cell, Row, Table, TableState},
};

use k9t_app::{PodTableMode, TableRow};

use crate::theme::Theme;

const MIN_WIDE_TABLE_WIDTH: u16 = 100;

fn status_style(theme: &Theme, status: &str) -> Style {
    match status {
        "Running" => theme.status_success(),
        "CrashLoopBackOff" | "Terminating" => theme.status_warning(),
        "Pending" => theme.status_warning(),
        _ => theme.fg_default(),
    }
}

/// Container status uses a slightly different mapping:
/// "Running" = green, "Waiting" or common wait reasons = yellow, "Terminated" = red-ish.
fn shorten_image(image: &str) -> String {
    if let Some((domain, path)) = image.split_once('/') {
        if domain.contains('.') {
            let parts: Vec<String> = domain
                .split('.')
                .filter(|s| !s.is_empty())
                .map(|s| s.chars().next().unwrap().to_string())
                .collect();
            let short_domain = parts.join(".");
            format!("{}/{}", short_domain, path)
        } else {
            image.to_string()
        }
    } else {
        image.to_string()
    }
}

/// Container status uses a slightly different mapping:
/// "Running" = green, "Waiting" or common wait reasons = yellow, "Terminated" = red-ish.
fn container_status_style(theme: &Theme, status: &str, is_init: bool) -> Style {
    if is_init && status == "Completed" {
        return theme.fg_muted();
    }
    match status {
        "Running" => theme.status_success(),
        "Completed" => theme.fg_muted(),
        "CrashLoopBackOff" => theme.status_error(),
        s if s == "Error" || s == "OOMKilled" || s == "ContainerCannotRun" => theme.status_error(),
        s if s.contains("Error") || s == "ImagePullBackOff" || s == "ErrImagePull" => {
            theme.status_error()
        }
        _ => theme.status_warning(),
    }
}

/// Compute column widths that auto-resize to fit content and terminal width.
/// Columns: NAMESPACE, NAME, READY, STATUS, RESTARTS, AGE (for pods)
/// Container rows show: (empty), NAME(indented), READY(✓/✗), STATUS, RESTARTS, AGE(inherited)
fn compute_compact_column_widths(rows: &[TableRow]) -> [Constraint; 6] {
    let header_lens: [u16; 6] = [9, 4, 5, 6, 8, 3]; // NAMESPACE, NAME, READY, STATUS, RESTARTS, AGE

    let data_max = if rows.is_empty() {
        header_lens
    } else {
        let mut maxes = header_lens;
        for row in rows {
            match row {
                TableRow::Pod { pod, .. } => {
                    // NAME gets 2 extra for the expand indicator "▸ " or "▾ "
                    maxes[0] = maxes[0].max(pod.namespace.len() as u16);
                    maxes[1] = maxes[1].max(pod.name.len() as u16 + 2);
                    maxes[2] = maxes[2].max(pod.ready.len() as u16);
                    maxes[3] = maxes[3].max(pod.status.len() as u16);
                    maxes[4] = maxes[4].max(pod.restarts.to_string().len() as u16);
                    maxes[5] = maxes[5].max(pod.age.len() as u16);
                }
                TableRow::Container { container, .. } => {
                    // Container name is indented with tree chars "│  ├─ " or "└─ "
                    let indented_len = container.name.len() as u16 + 6; // "  ├─ " prefix
                    maxes[1] = maxes[1].max(indented_len);
                    // READY: "✓" or "✗" = 1 char
                    maxes[2] = maxes[2].max(1);
                    // STATUS: container status string
                    maxes[3] = maxes[3].max(container.status.len() as u16);
                    // RESTARTS
                    maxes[4] = maxes[4].max(container.restart_count.to_string().len() as u16);
                    // AGE: same as pod (not shown for containers, but reserve space)
                }
            }
        }
        maxes
    };

    // 1-cell padding on each side of every column for readability
    let padded: [u16; 6] = data_max.map(|w| w.saturating_add(2));

    [
        Constraint::Length(padded[0]), // NAMESPACE — fixed to widest content
        Constraint::Min(padded[1]),    // NAME — absorbs all remaining width
        Constraint::Length(padded[2]), // READY
        Constraint::Length(padded[3]), // STATUS
        Constraint::Length(padded[4]), // RESTARTS
        Constraint::Length(padded[5]), // AGE
    ]
}

fn compute_wide_column_widths(rows: &[TableRow]) -> [Constraint; 9] {
    let header_lens: [u16; 9] = [9, 4, 5, 6, 8, 2, 4, 16, 3]; // NAMESPACE, NAME, READY, STATUS, RESTARTS, IP, NODE, IMAGE, AGE

    let data_max = if rows.is_empty() {
        header_lens
    } else {
        let mut maxes = header_lens;
        for row in rows {
            match row {
                TableRow::Pod { pod, .. } => {
                    maxes[0] = maxes[0].max(pod.namespace.len() as u16);
                    maxes[1] = maxes[1].max(pod.name.len() as u16 + 2);
                    maxes[2] = maxes[2].max(pod.ready.len() as u16);
                    maxes[3] = maxes[3].max(pod.status.len() as u16);
                    maxes[4] = maxes[4].max(pod.restarts.to_string().len() as u16);
                    maxes[5] = maxes[5].max(pod.pod_ip.len() as u16);
                    maxes[6] = maxes[6].max(pod.node_name.len() as u16);
                    maxes[7] = maxes[7].max(pod.container_details.first().map(|c| shorten_image(&c.image).len()).unwrap_or(0) as u16);
                    maxes[8] = maxes[8].max(pod.age.len() as u16);
                }
                TableRow::Container { container, .. } => {
                    let indented_len = container.name.len() as u16 + 6;
                    maxes[1] = maxes[1].max(indented_len);
                    maxes[2] = maxes[2].max(1);
                    maxes[3] = maxes[3].max(container.status.len() as u16);
                    maxes[4] = maxes[4].max(container.restart_count.to_string().len() as u16);
                    maxes[7] = maxes[7].max(shorten_image(&container.image).len() as u16);
                }
            }
        }
        maxes
    };

    let padded: [u16; 9] = data_max.map(|w| w.saturating_add(2));

    [
        Constraint::Length(padded[0].min(20)),
        Constraint::Min(padded[1]),
        Constraint::Length(padded[2]),
        Constraint::Length(padded[3].min(18)),
        Constraint::Length(padded[4]),
        Constraint::Length(padded[5].min(18)),
        Constraint::Length(padded[6].min(24)),
        Constraint::Length(padded[7].min(60)),
        Constraint::Length(padded[8]),
    ]
}

fn effective_table_mode(mode: PodTableMode, area_width: u16) -> PodTableMode {
    match mode {
        PodTableMode::Wide if area_width >= MIN_WIDE_TABLE_WIDTH => PodTableMode::Wide,
        _ => PodTableMode::Compact,
    }
}

pub fn render_pod_table(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    rows: &[TableRow],
    selected_index: usize,
    _title: &str,
    mode: PodTableMode,
    theme: &Theme,
) {
    let mode = effective_table_mode(mode, area.width);

    let header = match mode {
        PodTableMode::Compact => {
            let header_cells = ["NAMESPACE", "NAME", "READY", "STATUS", "RESTARTS", "AGE"]
                .into_iter()
                .map(|h| Cell::from(h).style(theme.title_style()));
            Row::new(header_cells).style(theme.title_style())
        }
        PodTableMode::Wide => {
            let header_cells = [
                "NAMESPACE",
                "NAME",
                "READY",
                "STATUS",
                "RESTARTS",
                "IP",
                "NODE",
                "IMAGE",
                "AGE",
            ]
            .into_iter()
            .map(|h| Cell::from(h).style(theme.title_style()));
            Row::new(header_cells).style(theme.title_style())
        }
    };

    let table_rows: Vec<Row> = rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let is_selected = i == selected_index;
            match row {
                TableRow::Pod { pod, expanded } => {
                    let expand_indicator = if *expanded { "▾ " } else { "▸ " };
                    let name_with_indicator = format!("{}{}", expand_indicator, pod.name);

                    let status_cell =
                        Cell::from(Span::styled(&pod.status, status_style(theme, &pod.status)));

                    let pod_image = pod.container_details.first().map(|c| shorten_image(&c.image)).unwrap_or_else(|| "-".to_string());

                    let cells = match mode {
                        PodTableMode::Compact => vec![
                            Cell::from(pod.namespace.as_str()),
                            Cell::from(name_with_indicator),
                            Cell::from(pod.ready.as_str()),
                            status_cell,
                            Cell::from(pod.restarts.to_string()),
                            Cell::from(pod.age.as_str()),
                        ],
                        PodTableMode::Wide => vec![
                            Cell::from(pod.namespace.as_str()),
                            Cell::from(name_with_indicator),
                            Cell::from(pod.ready.as_str()),
                            status_cell,
                            Cell::from(pod.restarts.to_string()),
                            Cell::from(pod.pod_ip.as_str()),
                            Cell::from(pod.node_name.as_str()),
                            Cell::from(pod_image),
                            Cell::from(pod.age.as_str()),
                        ],
                    };

                    Row::new(cells).style(if is_selected {
                        theme.selected_style()
                    } else {
                        theme.fg_default()
                    })
                }
                TableRow::Container {
                    container, is_last, ..
                } => {
                    let tree_prefix = if *is_last { "  └─ " } else { "  ├─ " };
                    let init_tag = if container.is_init { "Ⓘ " } else { "" };
                    let name_cell = format!("{}{}{}", tree_prefix, init_tag, container.name);

                    // Ready: ✓ or ✗
                    let ready_str = if container.ready { "✓" } else { "✗" };
                    let ready_style = if container.ready {
                        theme.status_success()
                    } else {
                        theme.status_error()
                    };

                    let status_cell = Cell::from(Span::styled(
                        &container.status,
                        container_status_style(theme, &container.status, container.is_init),
                    ));

                    let container_image = shorten_image(&container.image);

                    let cells = match mode {
                        PodTableMode::Compact => {
                            // Container rows show: (empty), NAME(indented), READY(✓/✗), STATUS, RESTARTS, (empty for AGE)
                            vec![
                                Cell::from(""),
                                Cell::from(name_cell),
                                Cell::from(Span::styled(ready_str, ready_style)),
                                status_cell,
                                Cell::from(container.restart_count.to_string()),
                                Cell::from(""),
                            ]
                        }
                        PodTableMode::Wide => vec![
                            Cell::from(""),
                            Cell::from(name_cell),
                            Cell::from(Span::styled(ready_str, ready_style)),
                            status_cell,
                            Cell::from(container.restart_count.to_string()),
                            Cell::from(""),
                            Cell::from(""),
                            Cell::from(container_image),
                            Cell::from(""),
                        ],
                    };

                    Row::new(cells).style(if is_selected {
                        theme.selected_style()
                    } else {
                        theme.fg_muted()
                    })
                }
            }
        })
        .collect();

    let table = match mode {
        PodTableMode::Compact => Table::new(table_rows, compute_compact_column_widths(rows)),
        PodTableMode::Wide => Table::new(table_rows, compute_wide_column_widths(rows)),
    }
    .header(header)
    .row_highlight_style(theme.selected_style());

    // Use TableState for proper scrolling — ratatui automatically scrolls
    // the viewport to keep the selected row visible.
    let mut state = TableState::default();
    state.select(Some(selected_index));

    let padded_table = table.style(theme.bg_surface());
    frame.render_stateful_widget(padded_table, area, &mut state);
}
