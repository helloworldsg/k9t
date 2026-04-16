use ratatui::{
    Frame,
    style::Modifier,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::theme::Theme;

pub fn render_header(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    context: &str,
    namespace: &str,
    theme: &Theme,
) {
    let now = jiff::Timestamp::now();
    let time_str = now.strftime("%H:%M:%S").to_string();

    let spans = vec![
        Span::styled("k9t", theme.accent_primary().add_modifier(Modifier::BOLD)),
        Span::styled(" ── ", theme.fg_muted()),
        Span::styled("context: ", theme.fg_muted()),
        Span::styled(context.to_string(), theme.fg_emphasis()),
        Span::styled(" ── ", theme.fg_muted()),
        Span::styled("namespace: ", theme.fg_muted()),
        Span::styled(namespace.to_string(), theme.fg_emphasis()),
        Span::styled(" ── ", theme.fg_muted()),
        Span::styled(time_str, theme.fg_muted()),
    ];

    let paragraph = Paragraph::new(Line::from(spans)).style(theme.bg_surface());

    frame.render_widget(paragraph, area);
}
