use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

use crate::theme::Theme;

/// An item shown in the command palette.
#[derive(Debug, Clone)]
pub struct CommandItem {
    /// The `:name` the user types (e.g. "ns", "pf").
    pub name: String,
    /// Short description for the right column.
    pub description: String,
    /// Whether this is a built-in or custom command.
    pub is_custom: bool,
}

/// Render a compact bottom-anchored command palette.
///
/// Shows `:query█` prompt on the last line, filtered results above it.
/// `filtered` — command items already filtered by fuzzy query.
/// `highlight_index` — which item is currently highlighted.
pub fn render_command_palette(
    frame: &mut Frame,
    area: Rect,
    query: &str,
    filtered: &[CommandItem],
    highlight_index: usize,
    theme: &Theme,
    blink_cursor: bool,
) {
    let cursor = if blink_cursor { "█" } else { " " };

    if area.height < 2 || filtered.is_empty() {
        // Not enough space or no results — just show the prompt line
        let prompt = Line::from(vec![
            Span::styled(":", theme.accent_primary().add_modifier(Modifier::BOLD)),
            Span::styled(query.to_string(), theme.fg_default()),
            Span::styled(cursor, theme.accent_primary()),
        ]);
        frame.render_widget(Paragraph::new(prompt).style(theme.bg_overlay()), area);
        return;
    }

    // How many result lines can we show? (reserve 1 for the prompt)
    let max_results = (area.height as usize)
        .saturating_sub(1)
        .min(filtered.len())
        .min(6);
    let scroll_offset = if highlight_index >= max_results {
        highlight_index - max_results + 1
    } else {
        0
    };

    let prompt_row = area.y + area.height.saturating_sub(1);
    let results_top = (prompt_row as usize).saturating_sub(max_results);
    let results_rect = Rect {
        x: area.x,
        y: results_top as u16,
        width: area.width,
        height: max_results as u16,
    };
    let prompt_rect = Rect {
        x: area.x,
        y: prompt_row,
        width: area.width,
        height: 1,
    };

    // Clear result area
    frame.render_widget(Clear, results_rect);

    // Render filtered results
    let lines: Vec<Line> = filtered
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(max_results)
        .map(|(i, item)| {
            let hl = i == highlight_index;
            let name_style = if hl {
                theme.accent_primary().add_modifier(Modifier::BOLD)
            } else if item.is_custom {
                theme.status_success()
            } else {
                theme.fg_default()
            };
            let desc_style = if hl {
                theme.fg_default()
            } else {
                theme.fg_muted()
            };
            let marker = if hl { " >" } else { "  " };
            Line::from(vec![
                Span::styled(marker, name_style),
                Span::styled(format!(":{:<10}", item.name), name_style),
                Span::styled(item.description.clone(), desc_style),
            ])
        })
        .collect();

    let list_para = Paragraph::new(lines).style(theme.bg_surface());
    frame.render_widget(list_para, results_rect);

    // Render prompt line
    let prompt = Line::from(vec![
        Span::styled(":", theme.accent_primary().add_modifier(Modifier::BOLD)),
        Span::styled(query.to_string(), theme.fg_default()),
        Span::styled(cursor, theme.accent_primary()),
    ]);
    frame.render_widget(
        Paragraph::new(prompt).style(theme.bg_surface()),
        prompt_rect,
    );
}
