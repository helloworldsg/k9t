use crossterm::event::{KeyEvent, MouseEvent};

#[derive(Debug)]
pub enum AppEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Tick,
    Resize(u16, u16),
    PodsUpdated,
}
