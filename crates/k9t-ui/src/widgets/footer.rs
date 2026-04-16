use ratatui::{
    Frame,
    style::Modifier,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::theme::Theme;

pub fn render_footer(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    mode_name: &str,
    context_hints: &[(&str, &str)],
    theme: &Theme,
) {
    let mut spans = Vec::new();

    for (key, label) in context_hints {
        if !spans.is_empty() {
            spans.push(Span::styled("  ", theme.fg_muted()));
        }
        spans.push(Span::styled(format!("[{key}]"), theme.fg_muted()));
        spans.push(Span::styled(*label, theme.fg_emphasis()));
    }

    let universal = [
        ("q", "uit"),
        ("/", "search"),
        (":", "cmd"),
        ("?", "help"),
        ("n", "s"),
        ("x", "ctx"),
    ];

    for (key, label) in &universal {
        if !spans.is_empty() {
            spans.push(Span::styled("  ", theme.fg_muted()));
        }
        spans.push(Span::styled(format!("[{key}]"), theme.fg_muted()));
        spans.push(Span::styled(*label, theme.fg_emphasis()));
    }

    let mode_span = Span::styled(
        format!(" {mode_name}"),
        theme.accent_primary().add_modifier(Modifier::BOLD),
    );
    spans.push(Span::styled(" ", theme.bg_base()));
    spans.push(mode_span);

    let paragraph = Paragraph::new(Line::from(spans)).style(theme.bg_base());

    frame.render_widget(paragraph, area);
}

pub fn render_footer_default(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    mode_name: &str,
    theme: &Theme,
) {
    render_footer(frame, area, mode_name, &[], theme);
}
