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
