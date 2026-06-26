use std::io::{self, Stdout};

use crossterm::{
    event::{
        DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
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
    bracketed_paste: bool,
    keyboard_enhancement: bool,
}

impl TerminalGuard {
    pub fn new() -> Result<Self> {
        enable_raw_mode()?;
        let raw_enabled = true;
        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnterAlternateScreen) {
            cleanup_setup(raw_enabled, false, false, false, false);
            return Err(error);
        }
        let alternate_screen = true;
        if let Err(error) = execute!(stdout, EnableMouseCapture) {
            cleanup_setup(raw_enabled, alternate_screen, false, false, false);
            return Err(error);
        }
        let mouse_capture = true;
        if let Err(error) = execute!(stdout, EnableBracketedPaste) {
            cleanup_setup(raw_enabled, alternate_screen, mouse_capture, false, false);
            return Err(error);
        }
        let bracketed_paste = true;
        if let Err(error) = execute!(stdout, keyboard_enhancement_flags()) {
            cleanup_setup(
                raw_enabled,
                alternate_screen,
                mouse_capture,
                bracketed_paste,
                false,
            );
            return Err(error);
        }
        let keyboard_enhancement = true;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = match Terminal::new(backend) {
            Ok(terminal) => terminal,
            Err(error) => {
                cleanup_setup(
                    raw_enabled,
                    alternate_screen,
                    mouse_capture,
                    bracketed_paste,
                    keyboard_enhancement,
                );
                return Err(error);
            }
        };
        if let Err(error) = terminal.hide_cursor() {
            cleanup_setup(
                raw_enabled,
                alternate_screen,
                mouse_capture,
                bracketed_paste,
                keyboard_enhancement,
            );
            return Err(error);
        }

        Ok(Self {
            terminal,
            restored: false,
            raw_enabled,
            alternate_screen,
            mouse_capture,
            bracketed_paste,
            keyboard_enhancement,
        })
    }

    pub fn terminal_mut(&mut self) -> &mut CrosstermTerminal {
        &mut self.terminal
    }

    pub fn suspend<R>(&mut self, action: impl FnOnce() -> io::Result<R>) -> Result<R> {
        self.suspend_terminal()?;
        let action_result = action();
        let resume_result = self.resume_terminal();
        match (action_result, resume_result) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        }
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
        if self.bracketed_paste {
            match execute!(self.terminal.backend_mut(), DisableBracketedPaste) {
                Ok(()) => self.bracketed_paste = false,
                Err(error) => capture_first_error(&mut first_error, error),
            }
        }
        if self.keyboard_enhancement {
            match execute!(self.terminal.backend_mut(), PopKeyboardEnhancementFlags) {
                Ok(()) => self.keyboard_enhancement = false,
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
            && !self.mouse_capture
            && !self.bracketed_paste
            && !self.keyboard_enhancement;
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn suspend_terminal(&mut self) -> Result<()> {
        let mut first_error = None;
        if self.mouse_capture {
            match execute!(self.terminal.backend_mut(), DisableMouseCapture) {
                Ok(()) => self.mouse_capture = false,
                Err(error) => capture_first_error(&mut first_error, error),
            }
        }
        if self.bracketed_paste {
            match execute!(self.terminal.backend_mut(), DisableBracketedPaste) {
                Ok(()) => self.bracketed_paste = false,
                Err(error) => capture_first_error(&mut first_error, error),
            }
        }
        if self.keyboard_enhancement {
            match execute!(self.terminal.backend_mut(), PopKeyboardEnhancementFlags) {
                Ok(()) => self.keyboard_enhancement = false,
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

        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn resume_terminal(&mut self) -> Result<()> {
        let mut first_error = None;
        if !self.raw_enabled {
            match enable_raw_mode() {
                Ok(()) => self.raw_enabled = true,
                Err(error) => capture_first_error(&mut first_error, error),
            }
        }
        if !self.alternate_screen {
            match execute!(self.terminal.backend_mut(), EnterAlternateScreen) {
                Ok(()) => self.alternate_screen = true,
                Err(error) => capture_first_error(&mut first_error, error),
            }
        }
        if !self.mouse_capture {
            match execute!(self.terminal.backend_mut(), EnableMouseCapture) {
                Ok(()) => self.mouse_capture = true,
                Err(error) => capture_first_error(&mut first_error, error),
            }
        }
        if !self.bracketed_paste {
            match execute!(self.terminal.backend_mut(), EnableBracketedPaste) {
                Ok(()) => self.bracketed_paste = true,
                Err(error) => capture_first_error(&mut first_error, error),
            }
        }
        if !self.keyboard_enhancement {
            match execute!(self.terminal.backend_mut(), keyboard_enhancement_flags()) {
                Ok(()) => self.keyboard_enhancement = true,
                Err(error) => capture_first_error(&mut first_error, error),
            }
        }
        capture_first(&mut first_error, self.terminal.hide_cursor());

        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

fn cleanup_setup(
    raw_enabled: bool,
    alternate_screen: bool,
    mouse_capture: bool,
    bracketed_paste: bool,
    keyboard_enhancement: bool,
) {
    let mut first_error = None;
    if keyboard_enhancement {
        let mut stdout = io::stdout();
        capture_first(
            &mut first_error,
            execute!(stdout, PopKeyboardEnhancementFlags),
        );
    }
    if bracketed_paste {
        let mut stdout = io::stdout();
        capture_first(&mut first_error, execute!(stdout, DisableBracketedPaste));
    }
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

fn keyboard_enhancement_flags() -> PushKeyboardEnhancementFlags {
    PushKeyboardEnhancementFlags(
        KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
            | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
            | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS,
    )
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
