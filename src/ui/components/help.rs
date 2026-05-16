use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::state::{App, ViewMode};

pub fn draw_help(frame: &mut Frame, app: &App, area: Rect) {
    let columns = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area.inner(ratatui::layout::Margin {
            vertical: 0,
            horizontal: 1,
        }));

    let (left, right) = help_columns(app);

    frame.render_widget(
        Paragraph::new(left)
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::RIGHT)),
        columns[0],
    );
    frame.render_widget(Paragraph::new(right).wrap(Wrap { trim: true }), columns[1]);
}

fn help_columns(app: &App) -> (Vec<Line<'static>>, Vec<Line<'static>>) {
    match app.view_mode {
        ViewMode::UnitList => unit_list_columns(app),
        ViewMode::LogView => log_view_columns(app),
        ViewMode::FileView => file_view_columns(app),
    }
}

fn unit_list_columns(app: &App) -> (Vec<Line<'static>>, Vec<Line<'static>>) {
    if app.open_filter_menu.is_some() {
        return (
            vec![shortcut("Esc/q", "Close")],
            vec![shortcut("Shown keys", "Pick one")],
        );
    }

    (
        vec![
            shortcut("j/k", "Move up/down"),
            shortcut("Ctrl+u/d", "Half page up/down"),
            shortcut("Ctrl+b/f", "Full page up/down"),
            shortcut("gg/G", "Top/Bottom"),
        ],
        vec![
            shortcut("/", "Search"),
            shortcut("a/n/o/p", "Toggle filters"),
            shortcut("Ctrl+r", "Reset filters"),
            shortcut("Esc/q", "Back"),
        ],
    )
}

fn log_view_columns(app: &App) -> (Vec<Line<'static>>, Vec<Line<'static>>) {
    if app.is_searching && app.view_mode == ViewMode::LogView {
        return (
            vec![shortcut("Esc", "Clear"), shortcut("Enter", "Keep")],
            vec![
                shortcut("Left/Right", "Move cursor"),
                shortcut("Backspace", "Delete"),
            ],
        );
    }

    if app.visual_line_select {
        return (
            vec![shortcut("Space", "Mark"), shortcut("y/Enter", "Yank")],
            vec![shortcut("Esc", "Cancel"), shortcut("j/k", "Move")],
        );
    }

    if app.visual_select {
        return (
            vec![shortcut("Space", "Toggle"), shortcut("y/Enter", "Yank")],
            vec![shortcut("Esc", "Cancel"), shortcut("j/k", "Move")],
        );
    }

    (
        vec![
            shortcut("Ctrl+r", "Refresh"),
            shortcut("v", "Select"),
            shortcut("V", "Line select"),
        ],
        vec![
            shortcut("e", "Export"),
            shortcut("/", "Search"),
            shortcut("n/N", "Next/Prev"),
            shortcut("Esc/q", "Back"),
        ],
    )
}

fn file_view_columns(app: &App) -> (Vec<Line<'static>>, Vec<Line<'static>>) {
    if app.is_searching {
        return (
            vec![shortcut("Esc", "Clear"), shortcut("Enter", "Keep")],
            vec![
                shortcut("Left/Right", "Move cursor"),
                shortcut("Backspace", "Delete"),
            ],
        );
    }

    if app.pending_edit_review.is_some() {
        return (
            vec![shortcut("a/Enter", "Apply")],
            vec![shortcut("d/Esc/q", "Discard")],
        );
    }

    (
        vec![
            shortcut("/", "Search"),
            shortcut("n/N", "Next/Prev"),
            shortcut("e", "Override edit"),
        ],
        vec![
            shortcut("E", "Replace edit"),
            shortcut("Ctrl+b/f", "Page up/down"),
            shortcut("Esc/q", "Back"),
        ],
    )
}

fn shortcut(key: &str, description: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{key}: "), Style::default().fg(Color::Cyan).bold()),
        Span::styled(description.to_string(), Style::default()),
    ])
}
