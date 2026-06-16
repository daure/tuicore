use std::io::{self, Stdout};

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use super::Result;

pub type CrosstermTerminal = Terminal<CrosstermBackend<Stdout>>;

pub struct TerminalGuard {
    terminal: CrosstermTerminal,
    restored: bool,
    raw_enabled: bool,
    alternate_screen: bool,
    mouse_capture: bool,
}

impl TerminalGuard {
    pub fn new() -> Result<Self> {
        enable_raw_mode()?;
        let raw_enabled = true;
        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnterAlternateScreen) {
            cleanup_setup(raw_enabled, false, false);
            return Err(error);
        }
        let alternate_screen = true;
        if let Err(error) = execute!(stdout, EnableMouseCapture) {
            cleanup_setup(raw_enabled, alternate_screen, false);
            return Err(error);
        }
        let mouse_capture = true;
        let backend = CrosstermBackend::new(stdout);
        let terminal = match Terminal::new(backend) {
            Ok(terminal) => terminal,
            Err(error) => {
                cleanup_setup(raw_enabled, alternate_screen, mouse_capture);
                return Err(error);
            }
        };

        Ok(Self {
            terminal,
            restored: false,
            raw_enabled,
            alternate_screen,
            mouse_capture,
        })
    }

    pub fn terminal_mut(&mut self) -> &mut CrosstermTerminal {
        &mut self.terminal
    }

    pub fn restore(&mut self) -> Result<()> {
        if self.restored {
            return Ok(());
        }

        let mut first_error = None;
        if self.mouse_capture {
            match execute!(self.terminal.backend_mut(), DisableMouseCapture) {
                Ok(()) => self.mouse_capture = false,
                Err(error) => capture_first_error(&mut first_error, error),
            }
        }
        if self.raw_enabled {
            match disable_raw_mode() {
                Ok(()) => self.raw_enabled = false,
                Err(error) => capture_first_error(&mut first_error, error),
            }
        }
        if self.alternate_screen {
            match execute!(self.terminal.backend_mut(), LeaveAlternateScreen) {
                Ok(()) => self.alternate_screen = false,
                Err(error) => capture_first_error(&mut first_error, error),
            }
        }
        capture_first(&mut first_error, self.terminal.show_cursor());

        self.restored = first_error.is_none()
            && !self.raw_enabled
            && !self.alternate_screen
            && !self.mouse_capture;
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

fn cleanup_setup(raw_enabled: bool, alternate_screen: bool, mouse_capture: bool) {
    let mut first_error = None;
    if mouse_capture {
        let mut stdout = io::stdout();
        capture_first(&mut first_error, execute!(stdout, DisableMouseCapture));
    }
    if alternate_screen {
        let mut stdout = io::stdout();
        capture_first(&mut first_error, execute!(stdout, LeaveAlternateScreen));
    }
    if raw_enabled {
        capture_first(&mut first_error, disable_raw_mode());
    }
}

fn capture_first(first_error: &mut Option<io::Error>, result: io::Result<()>) {
    if first_error.is_none() {
        if let Err(error) = result {
            capture_first_error(first_error, error);
        }
    }
}

fn capture_first_error(first_error: &mut Option<io::Error>, error: io::Error) {
    if first_error.is_none() {
        *first_error = Some(error);
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}
