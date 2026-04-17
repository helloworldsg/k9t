use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
};

use crate::theme::Theme;

pub fn render_container_picker(
    frame: &mut Frame,
    area: Rect,
    containers: &[String],
    picker_index: usize,
    pod_name: &str,
    intent: &str,
    theme: &Theme,
) {
    let popup_width = 40.min(area.width);
    let popup_height = (containers.len() as u16 + 4)
        .min(area.height * 3 / 5)
        .max(5);
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
        .title(format!(" {intent}: {pod_name} "))
        .title_alignment(Alignment::Center)
        .style(theme.bg_overlay());

    frame.render_widget(&block, popup_area);

    let inner = block.inner(popup_area);

    let visible_height = inner.height as usize;
    let scroll_offset = if picker_index >= visible_height {
        picker_index - visible_height + 1
    } else {
        0
    };

    let lines: Vec<Line> = containers
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_height)
        .map(|(i, name)| {
            let is_highlighted = i == picker_index;
            let style = if is_highlighted {
                theme.selected_style()
            } else {
                theme.fg_default()
            };

            let indicator = if is_highlighted { " > " } else { "   " };
            Line::from(Span::styled(format!("{indicator}{name}"), style))
        })
        .collect();

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}
