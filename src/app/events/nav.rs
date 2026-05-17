use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::state::context::{App, NavAction};

impl App {
    pub fn handle_nav_key(&mut self, key: KeyEvent) -> bool {
        if self.pending_nav_prefix == Some('g') {
            self.pending_nav_prefix = None;
            if key.code == KeyCode::Char('g') {
                self.perform_nav(NavAction::Top);
                return true;
            }
        }

        let action = match key.code {
            KeyCode::Char('j') | KeyCode::Down => Some(NavAction::Down),
            KeyCode::Char('k') | KeyCode::Up => Some(NavAction::Up),
            KeyCode::Char('G') => Some(NavAction::Bottom),
            KeyCode::Char('g') => {
                self.pending_nav_prefix = Some('g');
                return true;
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(NavAction::HalfPageUp)
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(NavAction::HalfPageDown)
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(NavAction::PageDown)
            }
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(NavAction::PageUp)
            }
            _ => None,
        };

        if let Some(action) = action {
            self.perform_nav(action);
            true
        } else {
            false
        }
    }
}
