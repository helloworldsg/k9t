use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph},
    Frame,
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
) {
    let title = format!(" {} / {} ", pod_name, container_name);

    // Calculate width: widest of title, search line, hint line, or widest action label
    let search_hint = if query.is_empty() {
        20
    } else {
        query.len() + 3
    };
    let hint_len = " [Enter]select  [j/k]nav  [Esc]cancel".len();
    let max_action_len = filtered_actions
        .iter()
        .map(|a| a.label().len() + 3) // " > " prefix
        .max()
        .unwrap_or(0)
        .max(20); // minimum for "No matching actions"
    let content_width = title
        .len()
        .max(search_hint)
        .max(hint_len)
        .max(max_action_len);
    // Add 2 for borders
    let popup_width = (content_width as u16 + 2).min(area.width);
    let action_lines = filtered_actions.len().max(1);
    let popup_height = (action_lines as u16 + 5).min(area.height * 3 / 5).max(7);

    let popup_x = area.width.saturating_sub(popup_width) / 2;
    let popup_y = area.height.saturating_sub(popup_height) / 2;

    let popup_area = Rect {
        x: area.x + popup_x,
        y: area.y + popup_y,
        width: popup_width,
        height: popup_height,
    };

    frame.render_widget(Clear, popup_area);

    let [search_area, content_area, hint_area] = Layout::vertical([
        Constraint::Length(1), // search input
        Constraint::Min(0),    // actions list (block goes here)
        Constraint::Length(1), // hint
    ])
    .areas(popup_area);

    // Outer block with title
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(Span::styled(&title, theme.accent_primary()))
        .title_alignment(Alignment::Center)
        .style(theme.bg_overlay());

    let inner = block.inner(content_area);
    frame.render_widget(block, content_area);

    // Search input above the block
    let search_text = if query.is_empty() {
        "\u{1f50d} type to filter...".to_string()
    } else {
        format!("{}\u{2588}", query) // block cursor
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

    let hint = " [Enter]select  [j/k]nav  [Esc]cancel";
    let hint_line = Line::from(Span::styled(hint, theme.fg_muted()));
    let hint_paragraph = Paragraph::new(hint_line).style(theme.bg_overlay());
    frame.render_widget(hint_paragraph, hint_area);
}
