use std::io::{Result, Stdout, stdout};

use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::Constraint,
    style::{Color, Style},
};

pub struct Tui {
    pub terminal: Terminal<CrosstermBackend<Stdout>>,
    pub active: bool,
}

impl Tui {
    pub fn enter() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
        terminal.hide_cursor()?;
        terminal.clear()?;
        Ok(Self {
            terminal,
            active: true,
        })
    }

    pub fn exit(&mut self) -> Result<()> {
        if self.active {
            execute!(self.terminal.backend_mut(), LeaveAlternateScreen)?;
            disable_raw_mode()?;
            self.terminal.show_cursor()?;
            self.active = false;
        }
        Ok(())
    }

    pub fn resume(&mut self) -> Result<()> {
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

pub const UNIT_COLUMN_CONSTRAINTS: [Constraint; 6] = [
    Constraint::Percentage(30),
    Constraint::Percentage(8),
    Constraint::Percentage(8),
    Constraint::Percentage(18),
    Constraint::Percentage(18),
    Constraint::Percentage(18),
];

pub const EDIT_REVIEW_MODAL_WIDTH: u16 = 50;
pub const EDIT_REVIEW_MODAL_HEIGHT: u16 = 33;
pub const AUTH_MODAL_WIDTH: u16 = 33;
pub const AUTH_MODAL_HEIGHT: u16 = 25;

pub fn keybind_style() -> Style {
    Style::default().fg(Color::Cyan).bold()
}

pub fn selection_style() -> Style {
    Style::default().bg(Color::DarkGray)
}

pub fn accent_style() -> Style {
    Style::default().fg(Color::Cyan)
}

pub fn search_query_style() -> Style {
    accent_style()
}

pub fn search_cursor_style() -> Style {
    Style::default().bg(Color::Yellow).fg(Color::Black).bold()
}

pub fn search_match_style() -> Style {
    Style::default().bg(Color::Yellow).fg(Color::Black)
}

pub fn modal_border_style() -> Style {
    accent_style()
}

pub fn section_header_style() -> Style {
    keybind_style()
}
