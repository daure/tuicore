use std::time::Duration;

use crossterm::event as crossterm_event;

use crate::event::TuiEvent;

use super::Result;

#[derive(Debug, Default)]
pub struct EventSource;

impl EventSource {
    pub fn new() -> Self {
        Self
    }

    pub fn poll(&mut self, timeout: Duration) -> Result<Option<TuiEvent>> {
        if !crossterm_event::poll(timeout)? {
            return Ok(None);
        }

        let event = crossterm_event::read()?;
        Ok(TuiEvent::try_from(event).ok())
    }
}
