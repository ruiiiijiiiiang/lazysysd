use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::{app::state::App, ui::render::render_scrollbar};

pub fn draw_file_view(frame: &mut Frame, app: &mut App, area: Rect) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

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
