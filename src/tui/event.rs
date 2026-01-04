use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, KeyEventKind, MouseEvent};
use std::time::Duration;

#[derive(Debug)]
pub enum Event {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Tick,
}

pub struct EventHandler;

impl EventHandler {
    pub fn new() -> Self {
        EventHandler
    }

    pub fn next(&self) -> std::io::Result<Event> {
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                CrosstermEvent::Key(key) => {
                    if key.kind == KeyEventKind::Press {
                        return Ok(Event::Key(key));
                    }
                }
                CrosstermEvent::Mouse(mouse) => {
                    return Ok(Event::Mouse(mouse));
                }
                _ => {}
            }
        }
        Ok(Event::Tick)
    }
}

