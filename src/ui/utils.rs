use std::io::{self, Stdout};

use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

pub struct Tui {
    pub terminal: Terminal<CrosstermBackend<Stdout>>,
    pub active: bool,
}

impl Tui {
    pub fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
        terminal.hide_cursor()?;
        terminal.clear()?;
        Ok(Self {
            terminal,
            active: true,
        })
    }

    pub fn exit(&mut self) -> io::Result<()> {
        if self.active {
            execute!(self.terminal.backend_mut(), LeaveAlternateScreen)?;
            disable_raw_mode()?;
            self.terminal.show_cursor()?;
            self.active = false;
        }
        Ok(())
    }

    pub fn resume(&mut self) -> io::Result<()> {
        if !self.active {
            enable_raw_mode()?;
            execute!(self.terminal.backend_mut(), EnterAlternateScreen)?;
            self.terminal.hide_cursor()?;
            self.terminal.clear()?;
            self.active = true;
        }
        Ok(())
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        let _ = self.exit();
    }
}
