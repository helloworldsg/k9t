use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph},
};

use crate::theme::Theme;
use k9t_app::mode::ContainerAction;

/// Render a centered container actions dialog.
pub fn render_container_actions(
    frame: &mut Frame,
    area: Rect,
    actions: &[ContainerAction],
    selected_index: usize,
    pod_name: &str,
    container_name: &str,
    theme: &Theme,
) {
    let action_count = actions.len();
    let popup_height = (action_count as u16 + 4).min(area.height * 3 / 5).max(5);
    let popup_width = 52.min(area.width);
    let popup_x = area.width.saturating_sub(popup_width) / 2;
    let popup_y = area.height.saturating_sub(popup_height) / 2;

    let popup_area = Rect {
        x: area.x + popup_x,
        y: area.y + popup_y,
        width: popup_width,
        height: popup_height,
    };

    frame.render_widget(Clear, popup_area);

    let [content_area, hint_area] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(popup_area);

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(format!(" Actions: {pod_name}/{container_name} "))
        .title_alignment(Alignment::Center)
        .style(theme.bg_overlay());

    let inner = block.inner(content_area);
    frame.render_widget(block, content_area);

    let visible_height = inner.height as usize;
    let scroll_offset = if selected_index >= visible_height {
        selected_index - visible_height + 1
    } else {
        0
    };

    let lines: Vec<Line> = actions
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_height)
        .map(|(i, action)| {
            let is_highlighted = i == selected_index;
            let shortcut = action.shortcut();
            let label = action.label();

            let indicator = if is_highlighted { " > " } else { "   " };
            let style = if is_highlighted {
                theme.selected_style()
            } else {
                theme.fg_default()
            };

            let shortcut_style = if is_highlighted {
                theme.accent_primary().add_modifier(Modifier::BOLD)
            } else {
                theme.fg_muted()
            };

            Line::from(vec![
                Span::styled(indicator.to_string(), style),
                Span::styled(format!("{shortcut} "), shortcut_style),
                Span::styled(label, style),
            ])
        })
        .collect();

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);

    let hint = " [Enter]select  [j/k]nav  [Esc]cancel  or press shortcut key";
    let hint_line = Line::from(Span::styled(hint, theme.fg_muted()));
    let hint_paragraph = Paragraph::new(hint_line).style(theme.bg_overlay());
    frame.render_widget(hint_paragraph, hint_area);
}
