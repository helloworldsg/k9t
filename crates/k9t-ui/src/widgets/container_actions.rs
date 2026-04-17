use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph},
};

use crate::theme::Theme;
use k9t_app::mode::ContainerAction;

/// Render a centered container actions dialog with type-to-filter.
#[allow(clippy::too_many_arguments)]
pub fn render_container_actions(
    frame: &mut Frame,
    area: Rect,
    filtered_actions: &[ContainerAction],
    selected_index: usize,
    query: &str,
    pod_name: &str,
    container_name: &str,
    theme: &Theme,
    blink_cursor: bool,
) {
    let title = format!(" {} / {} ", pod_name, container_name);

    // Calculate width: widest of title, search line, or widest action label
    let search_hint = if query.is_empty() {
        20
    } else {
        query.len() + 3
    };
    let max_action_len = filtered_actions
        .iter()
        .map(|a| a.label().len() + 3) // " > " prefix
        .max()
        .unwrap_or(0)
        .max(20); // minimum for "No matching actions"
    let content_width = title.len().max(search_hint).max(max_action_len);
    // Add 2 for borders
    let popup_width = (content_width as u16 + 2).min(area.width);
    let action_lines = filtered_actions.len().max(1);
    let popup_height = (action_lines as u16 + 4).min(area.height * 3 / 5).max(6);

    let popup_x = area.width.saturating_sub(popup_width) / 2;
    let popup_y = area.height.saturating_sub(popup_height) / 2;

    let popup_area = Rect {
        x: area.x + popup_x,
        y: area.y + popup_y,
        width: popup_width,
        height: popup_height,
    };

    frame.render_widget(Clear, popup_area);

    let search_area = Rect {
        x: popup_area.x,
        y: popup_area.y,
        width: popup_area.width,
        height: 1,
    };
    let content_area = Rect {
        x: popup_area.x,
        y: popup_area.y + 1,
        width: popup_area.width,
        height: popup_area.height.saturating_sub(1),
    };

    // Outer block with title
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(Span::styled(&title, theme.accent_primary()))
        .title_alignment(Alignment::Center)
        .style(theme.bg_overlay());

    let inner = block.inner(content_area);
    frame.render_widget(block, content_area);

    // Search input above the block with blinking cursor
    let cursor = if blink_cursor { "\u{2588}" } else { " " };
    let search_text = if query.is_empty() {
        "\u{1f50d} type to filter...".to_string()
    } else {
        format!("{}{}", query, cursor)
    };
    let search_style = if query.is_empty() {
        theme.fg_muted()
    } else {
        theme.fg_default()
    };
    let search = Paragraph::new(search_text).style(search_style);
    frame.render_widget(search, search_area);

    // Action list
    let visible_height = inner.height as usize;
    let scroll_offset = if selected_index >= visible_height {
        selected_index - visible_height + 1
    } else {
        0
    };

    if filtered_actions.is_empty() {
        let no_match = Paragraph::new(Line::from(Span::styled(
            " No matching actions",
            theme.fg_muted(),
        )));
        frame.render_widget(no_match, inner);
    } else {
        let lines: Vec<Line> = filtered_actions
            .iter()
            .enumerate()
            .skip(scroll_offset)
            .take(visible_height)
            .map(|(i, action)| {
                let is_highlighted = i == selected_index;
                let label = action.label();

                let indicator = if is_highlighted { " > " } else { "   " };
                let style = if is_highlighted {
                    theme.selected_style()
                } else {
                    theme.fg_default()
                };

                Line::from(vec![
                    Span::styled(indicator.to_string(), style),
                    Span::styled(label, style),
                ])
            })
            .collect();

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }
}
