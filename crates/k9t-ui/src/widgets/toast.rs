use ratatui::{
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::theme::Theme;

pub enum ToastType {
    Info,
    Success,
    Warning,
    Error,
}

pub fn render_toast(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    message: &str,
    toast_type: &ToastType,
    theme: &Theme,
) {
    let (icon, style) = match toast_type {
        ToastType::Info => ("ℹ", theme.status_info()),
        ToastType::Success => ("✓", theme.status_success()),
        ToastType::Warning => ("⚠", theme.status_warning()),
        ToastType::Error => ("✕", theme.status_error()),
    };

    let spans = vec![
        Span::styled(
            format!(" {icon} "),
            style.add_modifier(ratatui::style::Modifier::BOLD),
        ),
        Span::styled(message.to_string(), style),
    ];

    let paragraph = Paragraph::new(Line::from(spans)).style(theme.bg_overlay());

    frame.render_widget(paragraph, area);
}
