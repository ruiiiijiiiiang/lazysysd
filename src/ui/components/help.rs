use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::{
    app::state::context::{App, ViewMode},
    ui::utils::keybind_style,
};

pub fn draw_help(frame: &mut Frame, app: &App, area: Rect) {
    let columns = Layout::horizontal([
        Constraint::Percentage(33),
        Constraint::Percentage(34),
        Constraint::Percentage(33),
    ])
    .split(area.inner(ratatui::layout::Margin {
        vertical: 0,
        horizontal: 1,
    }));

    let (nav, action, external) = help_columns(app);

    frame.render_widget(
        Paragraph::new(nav)
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::RIGHT)),
        columns[0],
    );
    frame.render_widget(
        Paragraph::new(action)
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::RIGHT)),
        columns[1],
    );
    frame.render_widget(
        Paragraph::new(external).wrap(Wrap { trim: true }),
        columns[2],
    );
}

fn help_columns(app: &App) -> (Vec<Line<'static>>, Vec<Line<'static>>, Vec<Line<'static>>) {
    match app.view_mode {
        ViewMode::UnitList => unit_list_columns(app),
        ViewMode::LogView => log_view_columns(app),
        ViewMode::FileView => file_view_columns(app),
    }
}

fn nav_shortcuts() -> Vec<Line<'static>> {
    vec![
        shortcut("j/k", "Move up/down"),
        shortcut("Ctrl+u/d", "Half page up/down"),
        shortcut("Ctrl+b/f", "Full page up/down"),
        shortcut("gg/G", "Top/Bottom"),
    ]
}

fn unit_list_columns(app: &App) -> (Vec<Line<'static>>, Vec<Line<'static>>, Vec<Line<'static>>) {
    if app.unit_list.open_filter_menu.is_some() {
        return (
            vec![],
            vec![shortcut("Shown keys", "Pick one")],
            vec![shortcut("Esc/q", "Close")],
        );
    }

    if app.search.is_active {
        return (
            vec![shortcut("Left/Right", "Move cursor")],
            vec![shortcut("Backspace", "Delete"), shortcut("Enter", "Keep")],
            vec![shortcut("Esc", "Clear")],
        );
    }

    (
        nav_shortcuts(),
        vec![
            shortcut("/", "Search"),
            shortcut("y/p/a/n/o", "Toggle filters"),
            shortcut("Ctrl+r", "Reset filters"),
        ],
        vec![shortcut("Y", "Copy unit file path"), shortcut("q", "Quit")],
    )
}

fn log_view_columns(app: &App) -> (Vec<Line<'static>>, Vec<Line<'static>>, Vec<Line<'static>>) {
    if app.search.is_active {
        return (
            vec![shortcut("Left/Right", "Move cursor")],
            vec![shortcut("Backspace", "Delete"), shortcut("Enter", "Keep")],
            vec![shortcut("Esc", "Clear")],
        );
    }

    if app.log_view.visual_line_select || app.log_view.visual_select {
        return (
            nav_shortcuts(),
            vec![shortcut("Space", "Mark"), shortcut("y/Enter", "Yank")],
            vec![shortcut("Esc", "Cancel")],
        );
    }

    let mut action = vec![shortcut("Ctrl+r", "Refresh"), shortcut("/", "Search")];
    if !app.search.query.is_empty() {
        action.push(shortcut("n/N", "Next/prev"));
    }

    (
        nav_shortcuts(),
        action,
        vec![
            shortcut("v", "Select lines"),
            shortcut("V", "Select line blocks"),
            shortcut("e", "Open in editor"),
            shortcut("Esc/q", "Back"),
        ],
    )
}

fn file_view_columns(app: &App) -> (Vec<Line<'static>>, Vec<Line<'static>>, Vec<Line<'static>>) {
    if app.search.is_active {
        return (
            vec![shortcut("Left/Right", "Move cursor")],
            vec![shortcut("Backspace", "Delete"), shortcut("Enter", "Keep")],
            vec![shortcut("Esc", "Clear")],
        );
    }

    if app.pending_edit_review.is_some() {
        return (
            vec![],
            vec![],
            vec![shortcut("Enter", "Apply"), shortcut("Esc/q", "Discard")],
        );
    }

    let mut action = vec![shortcut("/", "Search")];
    if !app.search.query.is_empty() {
        action.push(shortcut("n/N", "Next/prev"));
    }

    (
        nav_shortcuts(),
        action,
        vec![
            shortcut("e", "Override edit"),
            shortcut("E", "Replace edit"),
            shortcut("Esc/q", "Back"),
        ],
    )
}

fn shortcut(key: &str, description: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(key.to_string(), keybind_style()),
        Span::styled(format!(": {description}"), Style::default()),
    ])
}
