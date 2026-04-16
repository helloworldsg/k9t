use crossterm::event::KeyEvent;

#[derive(Debug)]
pub enum AppEvent {
    Key(KeyEvent),
    Tick,
    Resize(u16, u16),
    PodsUpdated,
}
