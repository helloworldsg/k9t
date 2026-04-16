use ratatui::layout::{Constraint, Layout, Rect};

pub const MIN_WIDTH: u16 = 80;
pub const MIN_HEIGHT: u16 = 6;

pub fn is_terminal_too_small(area: Rect) -> bool {
    area.width < MIN_WIDTH || area.height < MIN_HEIGHT
}

pub struct AppLayout {
    pub header: Rect,
    pub namespace_bar: Rect,
    pub table: Rect,
    pub footer: Rect,
}

impl AppLayout {
    pub fn new(area: Rect) -> Self {
        let [header, namespace_bar, table, footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .areas(area);
        Self {
            header,
            namespace_bar,
            table,
            footer,
        }
    }
}
