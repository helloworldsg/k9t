use ratatui::layout::Rect;

use crate::layout::AppLayout;

pub struct LayoutCache {
    last_area: Rect,
    app_layout: Option<AppLayout>,
}

impl Default for LayoutCache {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutCache {
    pub fn new() -> Self {
        Self {
            last_area: Rect::default(),
            app_layout: None,
        }
    }

    pub fn app_layout(&mut self, area: Rect) -> &AppLayout {
        if area != self.last_area || self.app_layout.is_none() {
            self.app_layout = Some(AppLayout::new(area));
            self.last_area = area;
        }
        self.app_layout.as_ref().unwrap()
    }

    pub fn invalidate(&mut self) {
        self.app_layout = None;
    }
}
