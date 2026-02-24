//! Terminal setup, teardown, and the ratatui `Terminal` wrapper.
//!
//! This module owns the crossterm alternate-screen / raw-mode lifecycle.
//! It guarantees the terminal is restored even on panic via a `Drop` impl
//! on [`Tui`].

use std::io::{self, Stdout};

use crossterm::{
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::CrosstermBackend;

/// Owns a ratatui `Terminal` and ensures cleanup on drop.
pub struct Tui {
    pub terminal: ratatui::Terminal<CrosstermBackend<Stdout>>,
}

impl Tui {
    /// Enter raw mode, switch to the alternate screen, and create the
    /// ratatui terminal.
    pub fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(io::stdout());
        let terminal = ratatui::Terminal::new(backend)?;
        Ok(Self { terminal })
    }

    /// Restore the terminal to its original state.
    pub fn exit(&mut self) -> io::Result<()> {
        terminal::disable_raw_mode()?;
        execute!(io::stdout(), LeaveAlternateScreen)?;
        self.terminal.show_cursor()?;
        Ok(())
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        let _ = self.exit();
    }
}
