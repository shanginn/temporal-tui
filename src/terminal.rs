use std::io::{Stdout, stdout};

use anyhow::{Context, Result};
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

/// Concrete terminal used by the interactive runtime.
pub type AppTerminal = Terminal<CrosstermBackend<Stdout>>;

/// Owns raw/alternate-screen state and restores it on every exit path.
pub struct TerminalSession {
    terminal: AppTerminal,
    restored: bool,
}

impl TerminalSession {
    /// Enter raw mode and the alternate screen.
    ///
    /// # Errors
    ///
    /// Returns an error if raw mode, the alternate screen, or the Ratatui
    /// terminal cannot be initialized. Any state already entered is restored
    /// before returning the error.
    pub fn new() -> Result<Self> {
        enable_raw_mode().context("could not enable terminal raw mode")?;
        let mut output = stdout();
        if let Err(error) = execute!(output, EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(error).context("could not enter the terminal alternate screen");
        }
        let backend = CrosstermBackend::new(output);
        let terminal = match Terminal::new(backend) {
            Ok(terminal) => terminal,
            Err(error) => {
                let mut cleanup_output = stdout();
                let _ = execute!(cleanup_output, LeaveAlternateScreen);
                let _ = disable_raw_mode();
                return Err(error).context("could not initialize terminal");
            }
        };
        Ok(Self {
            terminal,
            restored: false,
        })
    }

    #[must_use]
    pub fn terminal_mut(&mut self) -> &mut AppTerminal {
        &mut self.terminal
    }

    /// Explicit restoration, also performed by `Drop`.
    ///
    /// # Errors
    ///
    /// Returns the first restoration error after attempting every cleanup step.
    pub fn restore(&mut self) -> Result<()> {
        if self.restored {
            return Ok(());
        }
        let results = [
            disable_raw_mode().context("could not disable terminal raw mode"),
            execute!(self.terminal.backend_mut(), LeaveAlternateScreen)
                .context("could not leave the terminal alternate screen"),
            self.terminal
                .show_cursor()
                .context("could not restore terminal cursor"),
        ];
        let mut first_error = None;
        for result in results {
            if let Err(error) = result
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }
        self.restored = first_error.is_none();
        first_error.map_or(Ok(()), Err)
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}
