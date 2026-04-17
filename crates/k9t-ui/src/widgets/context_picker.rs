use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
};

use crate::theme::Theme;

/// Render a fullscreen overlay for selecting a kube context (single-select).
///
/// - `available` — all context names from kubeconfig
/// - `current` — the currently active context name (highlighted with a marker)
/// - `picker_index` — index of the highlighted item in the filtered list
/// - `search` — current search/filter string
pub fn render_context_picker(
    frame: &mut Frame,
    area: Rect,
    available: &[String],
    current: Option<&str>,
    picker_index: usize,
    search: &str,
    theme: &Theme,
    blink_cursor: bool,
) {
    let filtered: Vec<&String> = if search.is_empty() {
        available.iter().collect()
    } else {
        available
            .iter()
            .filter(|ctx| ctx.to_lowercase().contains(&search.to_lowercase()))
            .collect()
    };

    let popup_width = (area.width as usize / 2).max(30);
    let popup_height = (area.height as usize * 3 / 5).max(5);
    let popup_x = (area.width as usize).saturating_sub(popup_width) / 2;
    let popup_y = (area.height as usize).saturating_sub(popup_height) / 2;

    let popup_area = Rect {
        x: area.x + popup_x as u16,
        y: area.y + popup_y as u16,
        width: popup_width as u16,
        height: popup_height as u16,
    };

    frame.render_widget(Clear, popup_area);

    let [search_area, content_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(popup_area);

    let block = Block::bordered()
        .title("Select Context")
        .title_alignment(Alignment::Center)
        .style(theme.bg_overlay());

    let inner = block.inner(content_area);
    frame.render_widget(block, content_area);

    // Search bar with blinking cursor
    let cursor = if blink_cursor { "█" } else { " " };
    let search_prompt = if search.is_empty() {
        Line::from(vec![
            Span::styled("\u{1f50d} ", theme.fg_muted()),
            Span::styled("type to filter...", theme.fg_muted()),
        ])
    } else {
        Line::from(vec![
            Span::styled("\u{1f50d} ", theme.fg_muted()),
            Span::styled(search.to_string(), theme.fg_default()),
            Span::styled(cursor, theme.accent_primary()),
        ])
    };
    frame.render_widget(
        Paragraph::new(search_prompt).style(theme.bg_overlay()),
        search_area,
    );

    let visible_height = inner.height as usize;
    let scroll_offset = if picker_index >= visible_height {
        picker_index - visible_height + 1
    } else {
        0
    };

    let lines: Vec<Line> = filtered
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_height)
        .map(|(i, ctx)| {
            let is_current = current == Some(ctx.as_str());
            let is_highlighted = i == picker_index;

            // Left marker: ● for current context, empty for others
            let marker = if is_current {
                "\u{25cf} " // ●
            } else {
                "  "
            };

            let style = if is_highlighted {
                theme.selected_style()
            } else if is_current {
                theme.accent_primary()
            } else {
                theme.fg_default()
            };

            Line::from(Span::styled(format!("{marker}{ctx}"), style))
        })
        .collect();

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}
