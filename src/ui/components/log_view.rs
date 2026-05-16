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
        let search_query = app.search_query.clone();
        let items: Vec<ListItem> = app
            .unit_logs
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let marker = if app.visual_line_select {
                    match line_range {
                        Some((start, end)) if start == end && i == start => {
                            Span::styled("┣ ", Style::default().fg(Color::Green))
                        }
                        Some((start, _)) if i == start => {
                            Span::styled("┏ ", Style::default().fg(Color::Green))
                        }
                        Some((_, end)) if i == end => {
                            Span::styled("┗ ", Style::default().fg(Color::Green))
                        }
                        Some((start, end)) if i >= start && i <= end => {
                            Span::styled("┃ ", Style::default().fg(Color::Green))
                        }
                        _ => Span::raw("┋ "),
                    }
                } else if app.visual_select {
                    if app.selected_log_lines.contains(&i) {
                        Span::styled("☑ ", Style::default().fg(Color::Green))
                    } else {
                        Span::raw("☐ ")
                    }
                } else {
                    Span::raw("")
                };
                let should_bold = if app.visual_line_select {
                    line_range
                        .map(|(start, end)| i >= start && i <= end)
                        .unwrap_or(false)
                } else {
                    app.selected_log_lines.contains(&i)
                };

                match line.as_bytes().into_text() {
                    Ok(t) => {
                        let mut l = t.lines.into_iter().next().unwrap_or_else(|| Line::from(""));
                        if !search_query.is_empty() && line.contains(&search_query) {
                            l = highlight_exact_match(l, &search_query);
                        }
                        if should_bold {
                            apply_selected_style(&mut l);
                        }
                        l.spans.insert(0, marker);
                        ListItem::new(l)
                    }
                    Err(_) => {
                        let mut l = Line::from(line.as_str());
                        if !search_query.is_empty() && line.contains(&search_query) {
                            l = highlight_exact_match(l, &search_query);
                        }
                        if should_bold {
                            apply_selected_style(&mut l);
                        }
                        l.spans.insert(0, marker);
                        ListItem::new(l)
                    }
                }
            })
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(Style::default().bg(Color::DarkGray));

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

fn apply_selected_style(line: &mut Line<'_>) {
    let bold = Style::default().bold().italic();
    line.style = line.style.patch(bold);
    for span in &mut line.spans {
        span.style = span.style.patch(bold);
    }
}
