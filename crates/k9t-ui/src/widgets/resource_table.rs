use ratatui::{
    Frame,
    layout::Constraint,
    style::Style,
    text::Span,
    widgets::{Cell, Row, Table, TableState},
};

use k9t_app::{PodTableMode, SortConfig, TableRow};

use crate::theme::Theme;

const MIN_WIDE_TABLE_WIDTH: u16 = 100;

fn make_header_cell<'a>(text: &'a str, column: &'a str, sort_config: &'a SortConfig) -> Cell<'a> {
    let sort_indicator = if sort_config.column() == column {
        if sort_config.is_descending() {
            " ▾"
        } else {
            " ▴"
        }
    } else {
        "  "
    };
    Cell::from(format!("{}{}", text, sort_indicator)).style(Style::default().bold())
}

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

fn make_boundaries(area: ratatui::layout::Rect, constraints: &[Constraint]) -> Vec<u16> {
    let mut current_x = 0;
    let mut boundaries = Vec::new();
    let table_width = area.width;

    for constraint in constraints {
        let width = match constraint {
            Constraint::Length(n) => *n,
            Constraint::Min(n) => {
                let used: u16 = constraints
                    .iter()
                    .map(|c| match c {
                        Constraint::Length(l) => *l,
                        Constraint::Min(_) => 0,
                        Constraint::Max(_) => 0,
                        Constraint::Percentage(_) => 0,
                        Constraint::Ratio(_, _) => 0,
                        Constraint::Fill(_) => 0,
                    })
                    .sum();
                (table_width.saturating_sub(used)).max(*n)
            }
            _ => 0,
        };
        current_x += width;
        boundaries.push(current_x);
    }
    boundaries
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
fn compute_compact_column_widths(rows: &[TableRow]) -> Vec<Constraint> {
    let header_lens: [u16; 6] = [11, 6, 5, 8, 8, 5]; // NAMESPACE, NAME, READY, STATUS, RESTARTS, AGE (with sort indicator space)

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

    vec![
        Constraint::Length(padded[0]), // NAMESPACE — fixed to widest content
        Constraint::Min(padded[1]),    // NAME — absorbs all remaining width
        Constraint::Length(padded[2]), // READY
        Constraint::Length(padded[3]), // STATUS
        Constraint::Length(padded[4]), // RESTARTS
        Constraint::Length(padded[5]), // AGE
    ]
}

fn compute_wide_column_widths(rows: &[TableRow]) -> Vec<Constraint> {
    let header_lens: [u16; 9] = [11, 6, 5, 8, 8, 4, 6, 18, 5]; // NAMESPACE, NAME, READY, STATUS, RESTARTS, IP, NODE, IMAGE, AGE (with sort indicator space)

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
                    maxes[7] = maxes[7].max(
                        pod.container_details
                            .first()
                            .map(|c| shorten_image(&c.image).len())
                            .unwrap_or(0) as u16,
                    );
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

    vec![
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

fn compute_wide_resources_column_widths(rows: &[TableRow]) -> Vec<Constraint> {
    let header_lens: [u16; 8] = [11, 6, 5, 8, 8, 12, 12, 5]; // NAMESPACE, NAME, READY, STATUS, RESTARTS, CPU, MEMORY, AGE (with sort indicator space)

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
                    maxes[5] = maxes[5].max(pod.cpu.len() as u16);
                    maxes[6] = maxes[6].max(pod.memory.len() as u16);
                    maxes[7] = maxes[7].max(pod.age.len() as u16);
                }
                TableRow::Container { container, .. } => {
                    let indented_len = container.name.len() as u16 + 6;
                    maxes[1] = maxes[1].max(indented_len);
                    maxes[2] = maxes[2].max(1);
                    maxes[3] = maxes[3].max(container.status.len() as u16);
                    maxes[4] = maxes[4].max(container.restart_count.to_string().len() as u16);
                }
            }
        }
        maxes
    };

    let padded: [u16; 8] = data_max.map(|w| w.saturating_add(2));

    vec![
        Constraint::Length(padded[0].min(20)),
        Constraint::Min(padded[1]),
        Constraint::Length(padded[2]),
        Constraint::Length(padded[3].min(18)),
        Constraint::Length(padded[4]),
        Constraint::Length(padded[5]),
        Constraint::Length(padded[6]),
        Constraint::Length(padded[7]),
    ]
}

fn effective_table_mode(mode: PodTableMode, area_width: u16) -> PodTableMode {
    match mode {
        PodTableMode::WideResources if area_width >= MIN_WIDE_TABLE_WIDTH => {
            PodTableMode::WideResources
        }
        PodTableMode::Wide if area_width >= MIN_WIDE_TABLE_WIDTH => PodTableMode::Wide,
        _ => PodTableMode::Compact,
    }
}

pub fn render_pod_table(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    rows: &[TableRow],
    selected_index: usize,
    mode: PodTableMode,
    sort_config: &SortConfig,
    theme: &Theme,
) -> Vec<u16> {
    let mode = effective_table_mode(mode, area.width);

    let constraints = match mode {
        PodTableMode::Compact => compute_compact_column_widths(rows),
        PodTableMode::Wide => compute_wide_column_widths(rows),
        PodTableMode::WideResources => compute_wide_resources_column_widths(rows),
    };

    let boundaries = make_boundaries(area, &constraints);

    let header = match mode {
        PodTableMode::Compact => {
            let columns = ["namespace", "name", "ready", "status", "restarts", "age"];
            let header_cells = ["NAMESPACE", "NAME", "READY", "STATUS", "RESTARTS", "AGE"]
                .iter()
                .zip(columns.iter())
                .map(|(h, col)| make_header_cell(h, col, sort_config));
            Row::new(header_cells).style(theme.title_style())
        }
        PodTableMode::Wide => {
            let columns = [
                "namespace",
                "name",
                "ready",
                "status",
                "restarts",
                "ip",
                "node",
                "image",
                "age",
            ];
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
            .iter()
            .zip(columns.iter())
            .map(|(h, col)| make_header_cell(h, col, sort_config));
            Row::new(header_cells).style(theme.title_style())
        }
        PodTableMode::WideResources => {
            let columns = [
                "namespace",
                "name",
                "ready",
                "status",
                "restarts",
                "cpu",
                "memory",
                "age",
            ];
            let header_cells = [
                "NAMESPACE",
                "NAME",
                "READY",
                "STATUS",
                "RESTARTS",
                "CPU",
                "MEMORY",
                "AGE",
            ]
            .iter()
            .zip(columns.iter())
            .map(|(h, col)| make_header_cell(h, col, sort_config));
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

                    let pod_image = pod
                        .container_details
                        .first()
                        .map(|c| shorten_image(&c.image))
                        .unwrap_or_else(|| "-".to_string());

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
                        PodTableMode::WideResources => vec![
                            Cell::from(pod.namespace.as_str()),
                            Cell::from(name_with_indicator),
                            Cell::from(pod.ready.as_str()),
                            status_cell,
                            Cell::from(pod.restarts.to_string()),
                            Cell::from(pod.cpu.as_str()),
                            Cell::from(pod.memory.as_str()),
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
                        PodTableMode::WideResources => vec![
                            Cell::from(""),
                            Cell::from(name_cell),
                            Cell::from(Span::styled(ready_str, ready_style)),
                            status_cell,
                            Cell::from(container.restart_count.to_string()),
                            Cell::from(""),
                            Cell::from(""),
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
        PodTableMode::WideResources => {
            Table::new(table_rows, compute_wide_resources_column_widths(rows))
        }
    }
    .header(header)
    .row_highlight_style(theme.selected_style());

    // Use TableState for proper scrolling — ratatui automatically scrolls
    // the viewport to keep the selected row visible.
    let mut state = TableState::default();
    state.select(Some(selected_index));

    let padded_table = table.style(theme.bg_surface());
    frame.render_stateful_widget(padded_table, area, &mut state);

    boundaries
}
