use ratatui::text::Line;

use crate::app::state::{App, ViewMode};

pub fn help_text(app: &App) -> Vec<Line<'static>> {
    let items = match app.view_mode {
        ViewMode::UnitList => {
            if app.open_filter_menu.is_some() {
                vec!["Esc/q: Close", "a: All", "Use shown keys", "Pick one"]
            } else {
                vec![
                    "j/k: Move",
                    "gg/G: Top/Bottom",
                    "/: Search",
                    "p/a/n/o: Filters",
                    "v: View File",
                    "Enter/l: Logs",
                    "s/t/r: Start/Stop/Restart",
                    "R, e/d: Reload, Enable/Disable",
                    "m/u: Mask/Unmask",
                    "q: Quit",
                ]
            }
        }
        ViewMode::LogView => {
            if app.log_search_mode {
                vec!["Esc: Clear", "Enter: Keep", "n/N: Next/Prev"]
            } else if !app.log_search_query.is_empty() {
                vec![
                    "Esc/q: Back",
                    "Ctrl+r: Refresh",
                    "v: Select",
                    "V/e: Line/Text editor",
                    "n/N: Next/Prev",
                    "/: Search",
                ]
            } else if app.visual_line_select {
                vec!["Esc: Cancel", "j/k: Move", "Space: Mark", "y/Enter: Yank"]
            } else if app.visual_select {
                vec!["Esc: Cancel", "j/k: Move", "Space: Toggle", "y/Enter: Yank"]
            } else {
                vec!["Esc/q: Back", "Ctrl+r: Refresh", "v: Select", "V/e: Line/Text editor"]
            }
        }
        ViewMode::FileView => {
            if app.file_search_mode {
                vec!["Esc: Clear", "Enter: Keep", "n/N: Next/Prev"]
            } else if !app.file_search_query.is_empty() {
                vec!["Esc/q: Back", "e/E: Edit", "n/N: Next/Prev", "/: Search"]
            } else if app.pending_edit_review.is_some() {
                vec!["a/Enter: Apply", "d/Esc/q: Discard"]
            } else {
                vec!["Esc/q: Back", "e: Override", "E: Replace", "/: Search"]
            }
        }
    };

    vec![Line::from(items.join("  |  "))]
}
