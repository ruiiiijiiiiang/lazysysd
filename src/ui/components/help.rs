use ratatui::text::Line;

use crate::app::state::{App, ViewMode};

pub fn help_text(app: &App) -> Vec<Line<'static>> {
    match app.view_mode {
        ViewMode::UnitList => {
            if app.open_filter_menu.is_some() {
                vec![
                    Line::from(" Esc/q: Close "),
                    Line::from(" a    : All   "),
                    Line::from(" Use shown keys"),
                    Line::from(" Pick one option"),
                ]
            } else {
                vec![
                    Line::from(" j/k, g/G, q: Move, Top/Bottom, Quit"),
                    Line::from(" Ctrl+r, /: Refresh, Search"),
                    Line::from(" p/a/n/o, v: Filters, View File"),
                    Line::from(" Enter/l: Logs"),
                    Line::from(" s/t/r: Start/Stop/Restart"),
                    Line::from(" R, e/d: Reload, Enable/Disable"),
                    Line::from(" m/u : Mask/Unmask"),
                ]
            }
        }
        ViewMode::LogView => {
            if app.visual_select {
                vec![
                    Line::from(" Esc : Cancel "),
                    Line::from(" j/k : Move   "),
                    Line::from(" Space: Toggle"),
                    Line::from(" y/Enter: Yank"),
                ]
            } else {
                vec![
                    Line::from(" Esc/q: Back "),
                    Line::from(" Ctrl+r: Refresh"),
                    Line::from(" v    : Select "),
                ]
            }
        }
        ViewMode::FileView => {
            if app.pending_edit_review.is_some() {
                vec![
                    Line::from(" a/Enter: Apply "),
                    Line::from(" d/Esc : Discard"),
                    Line::from(" q     : Discard"),
                ]
            } else {
                vec![
                    Line::from(" Esc/q: Back "),
                    Line::from(" e    : Override"),
                    Line::from(" E    : Replace "),
                ]
            }
        }
    }
}
