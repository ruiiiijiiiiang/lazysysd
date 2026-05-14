use ansi_to_tui::IntoText;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::{
    app::state::{App, ViewMode},
    systemd::auth::EmbeddedAuthFlow,
};

pub fn draw(frame: &mut Frame, app: &mut App) {
    let main_layout = Layout::vertical([
        Constraint::Length(3), // Search / Header
        Constraint::Min(10),   // List / Logs / File
        Constraint::Length(8), // Logs / Status
    ])
    .split(frame.area());

    // 1. Header / Search Bar
    if app.view_mode == ViewMode::UnitList {
        let search_style = if app.is_searching {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        let search_block = Block::default()
            .borders(Borders::ALL)
            .title(" Search (/) ")
            .border_style(search_style);
        let search_text = if app.search_query.is_empty() && !app.is_searching {
            Text::from("Type / to search...")
        } else {
            Text::from(app.search_query.as_str())
        };
        frame.render_widget(
            Paragraph::new(search_text).block(search_block),
            main_layout[0],
        );
    } else {
        let title = match app.view_mode {
            ViewMode::LogView => "Logs",
            ViewMode::FileView => "Unit File",
            _ => "",
        };
        let unit_name = app
            .list_state
            .selected()
            .and_then(|i| app.units.get(app.filtered_units[i]))
            .map(|u| u.name.as_str())
            .unwrap_or("Unknown");
        let header = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {}: {} (Esc to back) ", title, unit_name));
        frame.render_widget(
            Paragraph::new("Press Esc to return to the unit list").block(header),
            main_layout[0],
        );
    }

    // 2. Unit List, Log View, or File View
    match app.view_mode {
        ViewMode::UnitList => draw_unit_list(frame, app, main_layout[1]),
        ViewMode::LogView => draw_log_view(frame, app, main_layout[1]),
        ViewMode::FileView => draw_file_view(frame, app, main_layout[1]),
    }

    // 3. App Event Logs & Help
    let help_layout = Layout::horizontal([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(main_layout[2]);

    let logs_items: Vec<ListItem> = app
        .logs
        .iter()
        .rev()
        .map(|s| ListItem::new(s.as_str()))
        .collect();
    frame.render_widget(
        List::new(logs_items).block(Block::default().borders(Borders::ALL).title(" Event Log ")),
        help_layout[0],
    );

    let help_text = match app.view_mode {
        ViewMode::UnitList => vec![
            Line::from(" j/k: Navigate "),
            Line::from(" /  : Search   "),
            Line::from(" l/Enter: Logs "),
            Line::from(" v  : View File"),
            Line::from(" a  : Restart  "),
            Line::from(" r  : Refresh  "),
            Line::from(" q  : Quit     "),
        ],
        ViewMode::LogView => {
            if app.visual_select {
                vec![
                    Line::from(" Esc : Cancel  "),
                    Line::from(" j/k : Navigate "),
                    Line::from(" Space: Toggle  "),
                    Line::from(" y/Enter: Yank  "),
                ]
            } else {
                vec![
                    Line::from(" Esc/q: Back   "),
                    Line::from(" r    : Refresh"),
                    Line::from(" v    : Select  "),
                ]
            }
        }
        ViewMode::FileView => vec![Line::from(" Esc/q: Back   "), Line::from(" e    : Edit   ")],
    };
    frame.render_widget(
        Paragraph::new(help_text).block(Block::default().borders(Borders::ALL).title(" Help ")),
        help_layout[1],
    );

    // 4. Modal
    if let Some(auth) = &app.embedded_auth {
        render_auth_modal(frame, auth);
    }
}

fn draw_unit_list(frame: &mut Frame, app: &mut App, area: Rect) {
    app.last_area_height = area.height.saturating_sub(2);
    let list_block = Block::default().borders(Borders::ALL).title(format!(
        " Units ({}/{}) ",
        app.filtered_units.len(),
        app.units.len()
    ));

    if app.is_loading && app.units.is_empty() {
        frame.render_widget(
            Paragraph::new("Loading units...")
                .centered()
                .block(list_block),
            area,
        );
    } else {
        let items: Vec<ListItem> = app
            .filtered_units
            .iter()
            .map(|&i| {
                let unit = &app.units[i];
                let state_color = match unit.active_state.as_str() {
                    "active" => Color::Green,
                    "failed" => Color::Red,
                    "inactive" => Color::DarkGray,
                    _ => Color::White,
                };

                let header = Line::from(vec![
                    Span::styled(
                        format!("{:<40}", unit.name),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" {:<10}", unit.active_state),
                        Style::default().fg(state_color),
                    ),
                    Span::styled(
                        format!(" ({})", unit.sub_state),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]);
                let desc = Line::from(vec![Span::styled(
                    format!("  └─ {}", unit.description),
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                )]);
                ListItem::new(Text::from(vec![header, desc]))
            })
            .collect();

        let list = List::new(items)
            .block(list_block)
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(40, 40, 40))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        frame.render_stateful_widget(list, area, &mut app.list_state);
    }
}

fn draw_log_view(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Journal Logs ");
    app.last_area_height = area.height.saturating_sub(2); // Account for borders

    if app.is_loading && app.unit_logs.is_empty() {
        frame.render_widget(
            Paragraph::new("Fetching logs...").centered().block(block),
            area,
        );
    } else if app.unit_logs.is_empty() {
        frame.render_widget(
            Paragraph::new("No logs found or unauthorized.")
                .centered()
                .block(block),
            area,
        );
    } else {
        let items: Vec<ListItem> = app
            .unit_logs
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let marker = if app.visual_select {
                    if app.selected_log_lines.contains(&i) {
                        Span::styled("[X] ", Style::default().fg(Color::Green))
                    } else {
                        Span::raw("[ ] ")
                    }
                } else {
                    Span::raw("")
                };

                match line.as_bytes().into_text() {
                    Ok(t) => {
                        let mut l = t.lines.into_iter().next().unwrap_or_else(|| Line::from(""));
                        l.style = Style::default();
                        l.spans.insert(0, marker);
                        ListItem::new(l)
                    }
                    Err(_) => {
                        let mut l = Line::from(line.as_str());
                        l.spans.insert(0, marker);
                        ListItem::new(l)
                    }
                }
            })
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(Style::default().bg(Color::Rgb(60, 60, 60)));

        frame.render_stateful_widget(list, area, &mut app.log_state);
    }
}

fn draw_file_view(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Unit File: {} ", app.unit_file_path));

    if app.is_loading && app.unit_file_content.is_empty() {
        frame.render_widget(
            Paragraph::new("Loading unit file...")
                .centered()
                .block(block),
            area,
        );
    } else if app.unit_file_content.is_empty() {
        frame.render_widget(
            Paragraph::new("Failed to load unit file.")
                .centered()
                .block(block),
            area,
        );
    } else {
        let lines: Vec<Line> = app.unit_file_content.lines().map(Line::from).collect();

        frame.render_widget(
            Paragraph::new(lines)
                .block(block)
                .scroll((app.file_scroll, 0))
                .wrap(Wrap { trim: false }),
            area,
        );
    }
}

fn render_auth_modal(frame: &mut Frame, auth: &EmbeddedAuthFlow) {
    let area = centered_rect(80, 60, frame.area());
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title(" Authentication Required ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    frame.render_widget(block, area);

    let inner = area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let layout = Layout::vertical([Constraint::Min(5), Constraint::Length(3)]).split(inner);

    let prompt = if auth.pane.output.trim().is_empty() {
        "Waiting for polkit agent..."
    } else {
        auth.pane.output.as_str()
    };
    frame.render_widget(
        Paragraph::new(prompt)
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title(" Prompt ")),
        layout[0],
    );
    frame.render_widget(
        Paragraph::new("Enter password into terminal. Esc to cancel.")
            .centered()
            .block(Block::default().borders(Borders::ALL)),
        layout[1],
    );
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}
