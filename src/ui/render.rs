use ansi_to_tui::IntoText;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
};

use crate::{
    app::state::{App, FilterMenu, ViewMode},
    models::{EditReview, UnitInfo},
    systemd::auth::EmbeddedAuthFlow,
};

fn render_scrollbar(frame: &mut Frame, area: Rect, position: usize, content_length: usize) {
    if content_length <= area.height as usize {
        return;
    }

    let scrollbar = Scrollbar::default()
        .orientation(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"));

    let mut scrollbar_state =
        ScrollbarState::new(content_length.saturating_sub(area.height as usize))
            .position(position);

    frame.render_stateful_widget(
        scrollbar,
        area.inner(Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut scrollbar_state,
    );
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let main_layout = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(10),
        Constraint::Length(8),
    ])
    .split(frame.area());

    let mut filter_anchors = None;

    if app.view_mode == ViewMode::UnitList {
        filter_anchors = Some(draw_unit_header(frame, app, main_layout[0]));
    } else {
        let title = match app.view_mode {
            ViewMode::LogView => "Logs",
            ViewMode::FileView => "Unit File",
            ViewMode::UnitList => "",
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

    match app.view_mode {
        ViewMode::UnitList => draw_unit_list(frame, app, main_layout[1]),
        ViewMode::LogView => draw_log_view(frame, app, main_layout[1]),
        ViewMode::FileView => draw_file_view(frame, app, main_layout[1]),
    }

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

    frame.render_widget(
        Paragraph::new(help_text(app))
            .block(Block::default().borders(Borders::ALL).title(" Help ")),
        help_layout[1],
    );

    if let Some((active_rect, enablement_rect, load_rect)) = filter_anchors
        && let Some(menu) = app.open_filter_menu
    {
        let anchor = match menu {
            FilterMenu::Active => active_rect,
            FilterMenu::Enablement => enablement_rect,
            FilterMenu::Load => load_rect,
        };
        render_filter_menu(frame, app, menu, anchor, main_layout[1]);
    }

    if let Some(review) = &app.pending_edit_review {
        render_edit_review_modal(frame, review);
    }

    if let Some(auth) = &app.embedded_auth {
        render_auth_modal(frame, auth);
    }
}

fn draw_unit_header(frame: &mut Frame, app: &App, area: Rect) -> (Rect, Rect, Rect) {
    let header_layout = Layout::horizontal([
        Constraint::Percentage(40),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
    ])
    .split(area);

    let search_style = if app.is_searching {
        Style::default().fg(Color::Yellow)
    } else if !app.search_query.is_empty() {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    let search_text = if app.search_query.is_empty() && !app.is_searching {
        Text::from("Type / to search...")
    } else {
        Text::from(app.search_query.as_str())
    };
    frame.render_widget(
        Paragraph::new(search_text).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Search (/) ")
                .border_style(search_style),
        ),
        header_layout[0],
    );

    draw_filter_segment(frame, app, header_layout[1], FilterMenu::Active);
    draw_filter_segment(frame, app, header_layout[2], FilterMenu::Enablement);
    draw_filter_segment(frame, app, header_layout[3], FilterMenu::Load);

    (header_layout[1], header_layout[2], header_layout[3])
}

fn draw_filter_segment(frame: &mut Frame, app: &App, area: Rect, menu: FilterMenu) {
    let border_style = if app.open_filter_menu == Some(menu) {
        Style::default().fg(Color::Yellow)
    } else if app.filter_summary(menu) != "all" {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    let value_style = if app.filter_summary(menu) == "all" {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    };

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            app.filter_summary(menu),
            value_style,
        )))
        .centered()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(menu.segment_title())
                .border_style(border_style),
        ),
        area,
    );
}

fn help_text(app: &App) -> Vec<Line<'static>> {
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
                    Line::from(" a/n/o, v: Filters, View File"),
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

fn render_filter_menu(
    frame: &mut Frame,
    app: &App,
    menu: FilterMenu,
    anchor: Rect,
    list_area: Rect,
) {
    let options = app.filter_menu_options(menu);
    let content_width = options
        .iter()
        .map(|option| option.label.len() + 9)
        .max()
        .unwrap_or(18) as u16;
    let max_width = frame.area().width.saturating_sub(anchor.x).max(1);
    let width = anchor.width.max(content_width + 2).min(max_width);
    let y = anchor
        .y
        .saturating_add(anchor.height.saturating_sub(1))
        .max(list_area.y);
    let max_height = frame.area().height.saturating_sub(y).max(3);
    let height = (options.len() as u16 + 2).min(max_height);
    let area = Rect {
        x: anchor.x,
        y,
        width,
        height,
    };

    let items: Vec<ListItem> = options
        .into_iter()
        .map(|option| {
            let marker = if option.selected { "(O)" } else { "( )" };
            let style = if option.selected {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{marker} [{}] ", option.hotkey), style),
                Span::styled(option.label, style),
            ]))
        })
        .collect();

    frame.render_widget(Clear, area);
    frame.render_widget(
        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {} ", menu.title()))
                .border_style(Style::default().fg(Color::Yellow)),
        ),
        area,
    );
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
        let column_widths = unit_row_column_widths(area.width.saturating_sub(2));
        let items: Vec<ListItem> = app
            .filtered_units
            .iter()
            .map(|&i| {
                let unit = &app.units[i];
                ListItem::new(format_unit_row(unit, &column_widths))
            })
            .collect();

        let list = List::new(items).block(list_block).highlight_style(
            Style::default()
                .bg(Color::Rgb(40, 40, 40))
                .add_modifier(Modifier::BOLD),
        );

        frame.render_stateful_widget(list, area, &mut app.list_state);

        render_scrollbar(
            frame,
            area,
            app.list_state.selected().unwrap_or(0),
            app.filtered_units.len(),
        );
    }
}

fn format_unit_row(unit: &UnitInfo, widths: &[usize; 4]) -> Line<'static> {
    let active_sub = format!("{} ({})", unit.active_state, unit.sub_state);
    Line::from(vec![
        Span::styled(
            format_cell(&unit.name, widths[0], CellAlign::Left),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format_cell(&active_sub, widths[1], CellAlign::Center),
            Style::default().fg(active_state_color(&unit.active_state)),
        ),
        Span::styled(
            format_cell(&unit.enablement_state, widths[2], CellAlign::Center),
            Style::default().fg(enablement_state_color(&unit.enablement_state)),
        ),
        Span::styled(
            format_cell(&unit.load_state, widths[3], CellAlign::Center),
            Style::default().fg(load_state_color(&unit.load_state)),
        ),
    ])
}

fn unit_row_column_widths(total_width: u16) -> [usize; 4] {
    let columns = Layout::horizontal([
        Constraint::Percentage(40),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
    ])
    .split(Rect {
        x: 0,
        y: 0,
        width: total_width,
        height: 1,
    });

    [
        columns[0].width as usize,
        columns[1].width as usize,
        columns[2].width as usize,
        columns[3].width as usize,
    ]
}

fn active_state_color(state: &str) -> Color {
    match state {
        "active" => Color::Green,
        "failed" => Color::Red,
        "inactive" => Color::DarkGray,
        "activating" | "reloading" => Color::Yellow,
        "deactivating" => Color::LightYellow,
        "maintenance" => Color::Magenta,
        _ => Color::White,
    }
}

fn enablement_state_color(state: &str) -> Color {
    match state {
        "enabled" | "enabled-runtime" => Color::Green,
        "static" | "generated" | "alias" | "indirect" | "linked" | "linked-runtime" => Color::Cyan,
        "disabled" | "disabled-runtime" => Color::DarkGray,
        "masked" | "masked-runtime" | "invalid" => Color::Red,
        "transient" | "unknown" => Color::Yellow,
        _ => Color::White,
    }
}

fn load_state_color(state: &str) -> Color {
    match state {
        "loaded" => Color::Green,
        "not-found" => Color::Yellow,
        "bad-setting" | "error" | "masked" => Color::Red,
        _ => Color::White,
    }
}

#[derive(Clone, Copy)]
enum CellAlign {
    Left,
    Center,
}

fn format_cell(value: &str, width: usize, align: CellAlign) -> String {
    if width == 0 {
        return String::new();
    }

    let clipped = clip_text(value, width);
    let clipped_width = clipped.chars().count();
    let padding = width.saturating_sub(clipped_width);

    match align {
        CellAlign::Left => format!("{clipped}{:padding$}", "", padding = padding),
        CellAlign::Center => {
            let left = padding / 2;
            let right = padding.saturating_sub(left);
            format!(
                "{:left$}{clipped}{:right$}",
                "",
                "",
                left = left,
                right = right
            )
        }
    }
}

fn clip_text(value: &str, width: usize) -> String {
    let length = value.chars().count();
    if length <= width {
        return value.to_string();
    }

    if width <= 3 {
        return value.chars().take(width).collect();
    }

    let mut clipped: String = value.chars().take(width - 3).collect();
    clipped.push_str("...");
    clipped
}

fn highlight_unit_file(content: &str) -> Vec<Line<'static>> {
    let mut continued_value = false;

    content
        .lines()
        .map(|line| {
            let highlighted = highlight_unit_file_line(line, continued_value);
            continued_value = line_continues(line);
            highlighted
        })
        .collect()
}

fn highlight_unit_file_line(line: &str, continued_value: bool) -> Line<'static> {
    let trimmed = line.trim();

    if trimmed.is_empty() {
        return Line::from(line.to_string());
    }

    if trimmed.starts_with('#') || trimmed.starts_with(';') {
        return Line::from(Span::styled(
            line.to_string(),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        ));
    }

    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        return Line::from(Span::styled(
            line.to_string(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
    }

    if continued_value {
        return Line::from(Span::styled(
            line.to_string(),
            Style::default().fg(Color::White),
        ));
    }

    let indent_len = line.len().saturating_sub(line.trim_start().len());
    let (indent, rest) = line.split_at(indent_len);

    if let Some((key, value)) = rest.split_once('=') {
        return Line::from(vec![
            Span::raw(indent.to_string()),
            Span::styled(
                key.to_string(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("=".to_string(), Style::default().fg(Color::DarkGray)),
            Span::styled(value.to_string(), Style::default().fg(Color::White)),
        ]);
    }

    Line::from(line.to_string())
}

fn line_continues(line: &str) -> bool {
    line.trim_end().ends_with('\\')
}

fn draw_log_view(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Journal Logs ");
    app.last_area_height = area.height.saturating_sub(2);

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

        render_scrollbar(
            frame,
            area,
            app.log_state.selected().unwrap_or(0),
            app.unit_logs.len(),
        );
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
        let lines = highlight_unit_file(&app.unit_file_content);
        let content_length = lines.len();

        frame.render_widget(
            Paragraph::new(lines)
                .block(block)
                .scroll((app.file_scroll, 0))
                .wrap(Wrap { trim: false }),
            area,
        );

        render_scrollbar(frame, area, app.file_scroll as usize, content_length);
    }
}

fn render_edit_review_modal(frame: &mut Frame, review: &EditReview) {
    let area = centered_rect(72, 38, frame.area());
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title(format!(" Apply {} ", review.mode.action_label()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(block, area);

    let inner = area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let layout = Layout::vertical([Constraint::Min(4), Constraint::Length(3)]).split(inner);

    let body = vec![
        Line::from(format!("Unit: {}", review.unit_name)),
        Line::from(format!(
            "Mode: {} via systemctl edit",
            review.mode.action_label()
        )),
        Line::from("Draft returned from your editor and is ready to apply."),
        Line::from("Applying will request authorization and reload systemd automatically."),
    ];
    frame.render_widget(
        Paragraph::new(body)
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title(" Draft ")),
        layout[0],
    );
    frame.render_widget(
        Paragraph::new("a / Enter: apply    d / Esc: discard")
            .centered()
            .block(Block::default().borders(Borders::ALL)),
        layout[1],
    );
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
        Text::from("Waiting for polkit agent...")
    } else {
        match auth.pane.output.as_bytes().into_text() {
            Ok(t) => t,
            Err(_) => Text::from(auth.pane.output.as_str()),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_key_value_lines() {
        let line = highlight_unit_file_line("ExecStart=/usr/bin/true", false);

        assert_eq!(line.spans.len(), 4);
        assert_eq!(line.spans[1].content.as_ref(), "ExecStart");
        assert_eq!(line.spans[3].content.as_ref(), "/usr/bin/true");
        assert_eq!(line.spans[1].style.fg, Some(Color::Yellow));
    }

    #[test]
    fn highlights_comments_and_sections() {
        let comment = highlight_unit_file_line("# docs", false);
        let section = highlight_unit_file_line("[Service]", false);

        assert_eq!(comment.spans[0].style.fg, Some(Color::DarkGray));
        assert_eq!(section.spans[0].style.fg, Some(Color::Cyan));
    }

    #[test]
    fn treats_continued_lines_as_values() {
        let lines = highlight_unit_file("ExecStart=/usr/bin/foo \\\n    --flag");

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1].spans[0].style.fg, Some(Color::White));
    }
}
