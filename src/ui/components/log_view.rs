use ansi_to_tui::IntoText;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::{
    app::state::context::App,
    ui::{
        render::render_scrollbar,
        utils::{search_match_style, selection_style},
    },
};

pub fn draw_log_view(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Journal Logs ");

    if app.is_loading && app.log_view.logs.is_empty() {
        frame.render_widget(
            Paragraph::new("Fetching logs...").centered().block(block),
            area,
        );
    } else if app.log_view.logs.is_empty() {
        frame.render_widget(
            Paragraph::new("No logs found or unauthorized.")
                .centered()
                .block(block),
            area,
        );
    } else {
        let line_range = app.selected_log_line_range();
        let search_query = app.search.query.clone();
        let items: Vec<ListItem> = app
            .log_view
            .logs
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let marker = if app.log_view.visual_line_select {
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
                } else if app.log_view.visual_select {
                    if app.log_view.selected_lines.contains(&i) {
                        Span::styled("☑ ", Style::default().fg(Color::Green))
                    } else {
                        Span::raw("☐ ")
                    }
                } else {
                    Span::raw("")
                };
                let should_bold = if app.log_view.visual_line_select {
                    line_range
                        .map(|(start, end)| i >= start && i <= end)
                        .unwrap_or(false)
                } else {
                    app.log_view.selected_lines.contains(&i)
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
            .highlight_style(selection_style());

        frame.render_stateful_widget(list, area, &mut app.log_view.state);

        render_scrollbar(
            frame,
            area,
            app.log_view.state.selected().unwrap_or(0),
            app.log_view.logs.len(),
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
                style.patch(search_match_style()),
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
