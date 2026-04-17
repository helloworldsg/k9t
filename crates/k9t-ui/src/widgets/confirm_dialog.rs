use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph},
    Frame,
};

use crate::theme::Theme;

/// Render a centered confirmation dialog for destructive actions.
pub fn render_confirm_dialog(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    message: &str,
    resource: &str,
    theme: &Theme,
) {
    let popup_width = 52.min(area.width);
    let popup_height = 5.min(area.height);
    let popup_x = area.width.saturating_sub(popup_width) / 2;
    let popup_y = area.height.saturating_sub(popup_height) / 2;

    let popup_area = Rect {
        x: area.x + popup_x,
        y: area.y + popup_y,
        width: popup_width,
        height: popup_height,
    };

    frame.render_widget(Clear, popup_area);

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(format!(" {} ", title))
        .title_alignment(Alignment::Center)
        .style(theme.bg_overlay());

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let [msg_area, hint_area] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);

    let warning_style = theme.status_warning().add_modifier(Modifier::BOLD);
    let lines = vec![
        Line::from(Span::styled(
            format!(" {} {}", message, resource),
            warning_style,
        )),
        Line::from(""),
    ];
    let msg_para = Paragraph::new(lines).style(theme.bg_overlay());
    frame.render_widget(msg_para, msg_area);

    let hint = " [y] confirm  [Esc] cancel";
    let hint_para =
        Paragraph::new(Line::from(Span::styled(hint, theme.fg_muted()))).style(theme.bg_overlay());
    frame.render_widget(hint_para, hint_area);
}

/// Render a centered input dialog for text input (e.g. set image, port forward).
/// Uses a multi-line layout: label line(s) on top, input field on its own line,
/// so long resource names don't push the cursor off screen.
#[allow(clippy::too_many_arguments)]
pub fn render_input_dialog(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    label: &str,
    input: &str,
    placeholder: &str,
    hint: &str,
    theme: &Theme,
) {
    // Calculate width: ensure the input line fits label prefix + input with room to spare
    // Use at least 50 chars, or wider if the content needs it
    let min_content_width = 50;
    let input_line_width = label.len().max(placeholder.len()) + 4;
    let content_width = min_content_width.max(input_line_width);
    let popup_width = (content_width as u16 + 2).min(area.width); // +2 for borders
    let popup_height = 7.min(area.height);
    let popup_x = area.width.saturating_sub(popup_width) / 2;
    let popup_y = area.height.saturating_sub(popup_height) / 2;

    let popup_area = Rect {
        x: area.x + popup_x,
        y: area.y + popup_y,
        width: popup_width,
        height: popup_height,
    };

    frame.render_widget(Clear, popup_area);

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(Span::styled(format!(" {} ", title), theme.accent_primary()))
        .title_alignment(Alignment::Center)
        .style(theme.bg_overlay());

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    // Layout: label, blank, input, hint
    let [label_area, _, input_area, hint_area] = Layout::vertical([
        Constraint::Length(1), // label
        Constraint::Length(1), // blank line
        Constraint::Length(1), // input with cursor
        Constraint::Min(0),    // hint at bottom
    ])
    .areas(inner);

    // Label line (muted, showing context)
    let label_para = Paragraph::new(format!(" {}", label)).style(theme.fg_muted());
    frame.render_widget(label_para, label_area);

    // Input line: the typed text + block cursor + optional placeholder
    let inner_width = input_area.width as usize;
    // Scroll the input if it exceeds the visible width — show the tail end so the cursor is visible
    let visible_input = if input.len() + 2 > inner_width {
        let start = input.len().saturating_sub(inner_width.saturating_sub(3));
        &input[start..]
    } else {
        input
    };

    let mut spans = vec![
        Span::styled(visible_input.to_string(), theme.fg_default()),
        Span::styled("█", theme.accent_primary()),
    ];
    if input.is_empty() {
        spans.push(Span::styled(placeholder.to_string(), theme.fg_muted()));
    }
    let input_para = Paragraph::new(Line::from(spans)).style(theme.bg_overlay());
    frame.render_widget(input_para, input_area);

    let hint_para =
        Paragraph::new(Line::from(Span::styled(hint, theme.fg_muted()))).style(theme.bg_overlay());
    frame.render_widget(hint_para, hint_area);
}
