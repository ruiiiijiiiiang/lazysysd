use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::{app::state::App, models::UnitInfo, ui::render::render_scrollbar};

pub fn draw_unit_list(frame: &mut Frame, app: &mut App, area: Rect) {
    app.last_area_height = area.height.saturating_sub(2);
    let content_width = area.width.saturating_sub(2) as usize;
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
        let selected_index = app.selected_unit_index();
        let items: Vec<ListItem> = app
            .filtered_units
            .iter()
            .enumerate()
            .map(|(visible_index, &i)| {
                let unit = &app.units[i];
                ListItem::new(format_unit_row(
                    unit,
                    &column_widths,
                    content_width,
                    selected_index == Some(visible_index),
                ))
            })
            .collect();

        let list = List::new(items)
            .block(list_block)
            .highlight_style(Style::default().bg(Color::DarkGray).bold());

        frame.render_stateful_widget(list, area, &mut app.list_state);

        render_scrollbar(
            frame,
            area,
            selected_index.unwrap_or(0),
            app.filtered_units.len(),
        );
    }
}

fn format_unit_row(
    unit: &UnitInfo,
    widths: &[usize; 5],
    content_width: usize,
    is_selected: bool,
) -> Vec<Line<'static>> {
    let active_sub = format!("{} ({})", unit.active_state, unit.sub_state);
    let mut lines = vec![Line::from(vec![
        Span::styled(
            format_cell(&unit.name, widths[0], CellAlign::Left),
            Style::default().bold(),
        ),
        Span::styled(
            format_cell(&unit.scope, widths[1], CellAlign::Center),
            Style::default().fg(scope_color(&unit.scope)),
        ),
        Span::styled(
            format_cell(&active_sub, widths[2], CellAlign::Center),
            Style::default().fg(active_state_color(&unit.active_state)),
        ),
        Span::styled(
            format_cell(&unit.enablement_state, widths[3], CellAlign::Center),
            Style::default().fg(enablement_state_color(&unit.enablement_state)),
        ),
        Span::styled(
            format_cell(&unit.load_state, widths[4], CellAlign::Center),
            Style::default().fg(load_state_color(&unit.load_state)),
        ),
    ])];

    if is_selected {
        let detail = Line::from(vec![
            Span::raw(" ╟  "),
            Span::styled("Description: ", Style::default().bold()),
            Span::styled(unit.description.clone(), Style::default().fg(Color::Gray)),
            Span::raw("   "),
            Span::styled("Path: ", Style::default().bold()),
            Span::styled(unit.path.to_string(), Style::default().fg(Color::Gray)),
        ]);
        lines.push(clip_line(detail, content_width));

        let actions = Line::from(vec![
            Span::raw(" ╙  "),
            Span::styled("Actions: ", Style::default().bold()),
            Span::styled("l", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" logs   "),
            Span::styled("f", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" unit file   "),
            Span::styled("s", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" start   "),
            Span::styled("t", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" stop   "),
            Span::styled("r", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" restart   "),
            Span::styled("R", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" reload   "),
            Span::styled("e", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" enable   "),
            Span::styled("d", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" disable   "),
            Span::styled("m", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" mask   "),
            Span::styled("u", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" unmask"),
        ]);
        lines.push(clip_line(actions, content_width));
    }

    lines
}

fn unit_row_column_widths(total_width: u16) -> [usize; 5] {
    let columns = Layout::horizontal([
        Constraint::Percentage(30),
        Constraint::Percentage(15),
        Constraint::Percentage(18),
        Constraint::Percentage(18),
        Constraint::Percentage(19),
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
        columns[4].width as usize,
    ]
}

fn scope_color(scope: &str) -> Color {
    match scope {
        "global" => Color::Blue,
        "session" => Color::Cyan,
        _ => Color::White,
    }
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

fn clip_line(line: Line<'static>, width: usize) -> Line<'static> {
    if width == 0 {
        return Line::from(Vec::<Span<'static>>::new());
    }

    let mut spans = Vec::new();
    let mut remaining = width;

    for span in line.spans {
        if remaining == 0 {
            break;
        }

        let text = span.content.to_string();
        let text_width = text.chars().count();
        if text_width <= remaining {
            spans.push(Span::styled(text, span.style));
            remaining -= text_width;
            continue;
        }

        if remaining <= 3 {
            spans.push(Span::styled(
                text.chars().take(remaining).collect::<String>(),
                span.style,
            ));
            break;
        }

        let clipped: String = text.chars().take(remaining - 3).collect();
        spans.push(Span::styled(format!("{}...", clipped), span.style));
        break;
    }

    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::zvariant::OwnedObjectPath;

    use crate::models::UnitInfo;

    fn unit() -> UnitInfo {
        UnitInfo {
            name: "ssh.service".to_string(),
            description: "Secure Shell".to_string(),
            scope: "global".to_string(),
            load_state: "loaded".to_string(),
            active_state: "active".to_string(),
            enablement_state: "enabled".to_string(),
            sub_state: "running".to_string(),
            path: OwnedObjectPath::try_from("/test/unit/ssh").unwrap(),
        }
    }

    #[test]
    fn selected_row_gets_action_line_with_detail_prefix() {
        let lines = format_unit_row(&unit(), &[10, 10, 10, 10, 10], 80, true);

        assert_eq!(lines.len(), 3);
        assert!(lines[1].spans[0].content.starts_with(" ╟ "));
        assert!(lines[2].spans[0].content.starts_with(" ╙ "));
        assert!(
            lines[2]
                .spans
                .iter()
                .any(|span| span.content == "Actions: ")
        );
    }
}
