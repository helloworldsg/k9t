use std::collections::HashSet;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
};

use crate::theme::Theme;

pub fn render_namespace_picker(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    available: &[String],
    selected: &HashSet<String>,
    picker_index: usize,
    search: &str,
    theme: &Theme,
) {
    let all_selected = selected.is_empty();

    let filtered: Vec<&String> = if search.is_empty() {
        available.iter().collect()
    } else {
        available
            .iter()
            .filter(|ns| ns.to_lowercase().contains(&search.to_lowercase()))
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

    let [search_area, content_area, hint_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(popup_area);

    let block = Block::bordered()
        .title("Select Namespaces")
        .title_alignment(Alignment::Center)
        .style(theme.bg_overlay());

    let inner = block.inner(content_area);
    frame.render_widget(block, content_area);

    // Search bar
    let search_prompt = if search.is_empty() {
        Line::from(vec![
            Span::styled(" Filter: ", theme.fg_muted()),
            Span::styled("type to search…", theme.fg_muted()),
        ])
    } else {
        Line::from(vec![
            Span::styled(" Filter: ", theme.fg_muted()),
            Span::styled(search.to_string(), theme.fg_default()),
            Span::styled("█", theme.accent_primary()),
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
        .map(|(i, ns)| {
            let is_selected = all_selected || selected.contains(*ns);
            let is_highlighted = i == picker_index;
            let indicator = if is_selected {
                "\u{25cf} "
            } else {
                "\u{25cb} "
            };

            let style = if is_highlighted {
                theme.selected_style()
            } else if is_selected {
                theme.accent_primary()
            } else {
                theme.fg_muted()
            };

            Line::from(Span::styled(format!("{indicator}{ns}"), style))
        })
        .collect();

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);

    let hint = " [Space]toggle  [a]ll  [Enter]apply  [Esc]cancel  type to filter";
    let hint_line = Line::from(Span::styled(hint, theme.fg_muted()));
    let hint_paragraph = Paragraph::new(hint_line).style(theme.bg_overlay());
    frame.render_widget(hint_paragraph, hint_area);
}
