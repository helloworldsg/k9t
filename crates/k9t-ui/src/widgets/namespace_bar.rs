use std::collections::HashSet;

use ratatui::{
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::theme::Theme;

pub fn render_namespace_bar(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    selected: &HashSet<String>,
    available: &[String],
    theme: &Theme,
) {
    let all_selected = selected.is_empty();

    let mut spans = Vec::new();
    spans.push(Span::styled("Namespaces ", theme.title_style()));

    for (i, ns) in available.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" ", theme.fg_muted()));
        }
        let is_selected = all_selected || selected.contains(ns);
        if is_selected {
            spans.push(Span::styled(
                format!("\u{25cf}{ns}"),
                theme.accent_primary(),
            ));
        } else {
            spans.push(Span::styled(format!("\u{25cb}{ns}"), theme.fg_muted()));
        }
    }

    let paragraph = Paragraph::new(Line::from(spans)).style(theme.bg_surface());

    frame.render_widget(paragraph, area);
}
