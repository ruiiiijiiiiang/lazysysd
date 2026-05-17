use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::{app::state::context::App, ui::render::render_scrollbar};

pub fn draw_file_view(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Unit File: {} ", app.file_view.path));

    if app.is_loading && app.file_view.content.is_empty() {
        frame.render_widget(
            Paragraph::new("Loading unit file...")
                .centered()
                .block(block),
            area,
        );
    } else if app.file_view.content.is_empty() {
        frame.render_widget(
            Paragraph::new("Failed to load unit file.")
                .centered()
                .block(block),
            area,
        );
    } else {
        let lines =
            highlight_unit_file_with_search(&app.file_view.content, app.search.query.as_str());
        let content_length = lines.len();

        frame.render_widget(
            Paragraph::new(lines)
                .block(block)
                .scroll((app.file_view.scroll, 0))
                .wrap(Wrap { trim: false }),
            area,
        );

        render_scrollbar(frame, area, app.file_view.scroll as usize, content_length);
    }
}

fn highlight_unit_file_with_search(content: &str, search_query: &str) -> Vec<Line<'static>> {
    let mut continued_value = false;

    content
        .lines()
        .map(|line| {
            let highlighted =
                highlight_unit_file_line_with_search(line, continued_value, search_query);
            continued_value = line_continues(line);
            highlighted
        })
        .collect()
}

fn highlight_unit_file_line_with_search(
    line: &str,
    continued_value: bool,
    search_query: &str,
) -> Line<'static> {
    let trimmed = line.trim();

    if trimmed.is_empty() {
        return highlight_exact_matches(Line::from(line.to_string()), search_query);
    }

    if trimmed.starts_with('#') || trimmed.starts_with(';') {
        return highlight_exact_matches(
            Line::from(Span::styled(
                line.to_string(),
                Style::default().fg(Color::DarkGray).italic(),
            )),
            search_query,
        );
    }

    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        return highlight_exact_matches(
            Line::from(Span::styled(
                line.to_string(),
                Style::default().fg(Color::Cyan).bold(),
            )),
            search_query,
        );
    }

    if continued_value {
        return highlight_exact_matches(
            Line::from(Span::styled(
                line.to_string(),
                Style::default().fg(Color::White),
            )),
            search_query,
        );
    }

    let indent_len = line.len().saturating_sub(line.trim_start().len());
    let (indent, rest) = line.split_at(indent_len);

    if let Some((key, value)) = rest.split_once('=') {
        return highlight_exact_matches(
            Line::from(vec![
                Span::raw(indent.to_string()),
                Span::styled(key.to_string(), Style::default().fg(Color::Yellow).bold()),
                Span::styled("=".to_string(), Style::default().fg(Color::DarkGray)),
                Span::styled(value.to_string(), Style::default().fg(Color::White)),
            ]),
            search_query,
        );
    }

    highlight_exact_matches(Line::from(line.to_string()), search_query)
}

fn highlight_exact_matches(line: Line<'static>, query: &str) -> Line<'static> {
    if query.is_empty() {
        return line;
    }

    let mut spans = Vec::new();
    for span in line.spans {
        let content = span.content.into_owned();
        let style = span.style;

        if !content.contains(query) {
            spans.push(Span::styled(content, style));
            continue;
        }

        let mut remaining = content.as_str();
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

fn line_continues(line: &str) -> bool {
    line.trim_end().ends_with('\\')
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    #[test]
    fn highlights_key_value_lines() {
        let line = highlight_unit_file_line_with_search("ExecStart=/usr/bin/true", false, "");

        assert_eq!(line.spans.len(), 4);
        assert_eq!(line.spans[1].content.as_ref(), "ExecStart");
        assert_eq!(line.spans[3].content.as_ref(), "/usr/bin/true");
        assert_eq!(line.spans[1].style.fg, Some(Color::Yellow));
    }

    #[test]
    fn highlights_comments_and_sections() {
        let comment = highlight_unit_file_line_with_search("# docs", false, "");
        let section = highlight_unit_file_line_with_search("[Service]", false, "");

        assert_eq!(comment.spans[0].style.fg, Some(Color::DarkGray));
        assert_eq!(section.spans[0].style.fg, Some(Color::Cyan));
    }

    #[test]
    fn treats_continued_lines_as_values() {
        let lines = highlight_unit_file_with_search(
            concat!("ExecStart=/usr/bin/foo \\", "\n    --flag"),
            "",
        );

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1].spans[0].style.fg, Some(Color::White));
    }
}
