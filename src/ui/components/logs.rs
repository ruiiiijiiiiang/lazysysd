use ansi_to_tui::IntoText;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::{app::state::App, ui::render::render_scrollbar};

pub fn draw_log_view(frame: &mut Frame, app: &mut App, area: Rect) {
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
        let line_range = app.selected_log_line_range();
        let search_query = app.log_search_query.clone();
        let items: Vec<ListItem> = app
            .unit_logs
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let marker = if app.visual_line_select {
                    match app.selected_log_line_marks.iter().position(|&mark| mark == i) {
                        Some(0) => Span::styled("[A] ", Style::default().fg(Color::Green)),
                        Some(1) => Span::styled("[B] ", Style::default().fg(Color::Green)),
                        _ => {
                            if let Some((start, end)) = line_range {
                                if i >= start && i <= end {
                                    Span::styled("[=] ", Style::default().fg(Color::Green))
                                } else {
                                    Span::raw("[ ] ")
                                }
                            } else {
                                Span::raw("[ ] ")
                            }
                        }
                    }
                } else if app.visual_select {
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
                        if !search_query.is_empty() && line.contains(&search_query) {
                            l = highlight_exact_match(l, &search_query);
                        }
                        ListItem::new(l)
                    }
                    Err(_) => {
                        let mut l = Line::from(line.as_str());
                        l.spans.insert(0, marker);
                        if !search_query.is_empty() && line.contains(&search_query) {
                            l = highlight_exact_match(l, &search_query);
                        }
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

fn highlight_exact_match(line: Line<'_>, query: &str) -> Line<'static> {
    let mut spans = Vec::new();
    for span in line.spans {
        let content = span.content;
        let style = span.style;
        if query.is_empty() || !content.contains(query) {
            spans.push(Span::styled(content.to_string(), style));
            continue;
        }

        let mut remaining = content.as_ref();
        while let Some(pos) = remaining.find(query) {
            let (before, after) = remaining.split_at(pos);
            if !before.is_empty() {
                spans.push(Span::styled(before.to_string(), style));
            }
            spans.push(Span::styled(
                query.to_string(),
                style.bg(Color::Yellow).fg(Color::Black),
            ));
            remaining = &after[query.len()..];
        }
        if !remaining.is_empty() {
            spans.push(Span::styled(remaining.to_string(), style));
        }
    }

    Line::from(spans)
}
