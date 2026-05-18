use ratatui::{
    Frame,
    layout::{Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::{
    app::state::context::App,
    models::{UnitInfo, UnitType},
    ui::{
        render::render_scrollbar,
        utils::{UNIT_COLUMN_CONSTRAINTS, keybind_style, selection_style},
    },
};

pub fn draw_unit_list(frame: &mut Frame, app: &mut App, area: Rect) {
    let content_width = area.width.saturating_sub(2) as usize;
    let list_block = Block::default().borders(Borders::ALL).title(format!(
        " Units ({}/{}) ",
        app.unit_list.filtered_indices.len(),
        app.unit_list.units.len()
    ));

    if app.is_loading && app.unit_list.units.is_empty() {
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
            .unit_list
            .filtered_indices
            .iter()
            .enumerate()
            .map(|(visible_index, &i)| {
                let unit = &app.unit_list.units[i];
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
            .highlight_style(selection_style().bold());

        frame.render_stateful_widget(list, area, &mut app.unit_list.state);

        render_scrollbar(
            frame,
            area,
            selected_index.unwrap_or(0),
            app.unit_list.filtered_indices.len(),
        );
    }
}

fn format_unit_row(
    unit: &UnitInfo,
    widths: &[usize; 6],
    content_width: usize,
    is_selected: bool,
) -> Vec<Line<'static>> {
    let active_sub = format!("{} ({})", unit.active_state, unit.sub_state);
    let unit_type = UnitType::from_unit_name(&unit.name);
    let mut lines = vec![Line::from(vec![
        Span::styled(
            format_cell(&unit.name, widths[0], CellAlign::Left),
            Style::default().bold(),
        ),
        Span::styled(
            format_cell(unit_type.as_str(), widths[1], CellAlign::Center),
            Style::default().fg(unit_type.color()),
        ),
        Span::styled(
            format_cell(unit.scope.as_str(), widths[2], CellAlign::Center),
            Style::default().fg(unit.scope.color()),
        ),
        Span::styled(
            format_cell(&active_sub, widths[3], CellAlign::Center),
            Style::default().fg(unit.active_state.color()),
        ),
        Span::styled(
            format_cell(unit.enablement_state.as_str(), widths[4], CellAlign::Center),
            Style::default().fg(unit.enablement_state.color()),
        ),
        Span::styled(
            format_cell(unit.load_state.as_str(), widths[5], CellAlign::Center),
            Style::default().fg(unit.load_state.color()),
        ),
    ])];

    if is_selected {
        let detail = Line::from(vec![
            Span::raw(" ╟  "),
            Span::styled("Description: ", Style::default().bold()),
            Span::styled(unit.description.clone(), Style::default().fg(Color::White)),
            Span::styled("   Unit file path: ", Style::default().bold()),
            Span::styled(
                if unit.fragment_path.is_empty() {
                    "N/A".to_string()
                } else {
                    unit.fragment_path.clone()
                },
                Style::default().fg(Color::White),
            ),
        ]);
        lines.push(clip_line(detail, content_width));

        let actions_col0 = vec![
            Span::styled("l/Enter", keybind_style()),
            Span::raw(" view log  "),
            Span::styled("f", keybind_style()),
            Span::raw(" view unit file"),
        ];

        let actions_col2 = vec![
            Span::styled("s", keybind_style()),
            Span::raw(" start "),
            Span::styled("t", keybind_style()),
            Span::raw(" stop "),
            Span::styled("r", keybind_style()),
            Span::raw(" restart"),
        ];

        let actions_col3 = vec![
            Span::styled("e", keybind_style()),
            Span::raw(" enable "),
            Span::styled("d", keybind_style()),
            Span::raw(" disable "),
            Span::styled("m", keybind_style()),
            Span::raw(" mask "),
            Span::styled("u", keybind_style()),
            Span::raw(" unmask"),
        ];

        let actions_col4 = vec![
            Span::styled("R", keybind_style()),
            Span::raw(" reload "),
            Span::styled("x", keybind_style()),
            Span::raw(" reset-failed"),
        ];

        let mut actions_spans = vec![
            Span::raw(" ╙  "),
            Span::styled("Action: ", Style::default().bold()),
        ];

        let prefix_len = " ╙  Action: ".len();
        actions_spans.extend(format_spans_cell(
            actions_col0,
            widths[0].saturating_sub(prefix_len),
            CellAlign::Left,
        ));
        actions_spans.push(Span::raw(format!("{:width$}", "", width = widths[1])));
        actions_spans.extend(format_spans_cell(vec![], widths[2], CellAlign::Center));
        actions_spans.extend(format_spans_cell(
            actions_col2,
            widths[3],
            CellAlign::Center,
        ));
        actions_spans.extend(format_spans_cell(
            actions_col3,
            widths[4],
            CellAlign::Center,
        ));
        actions_spans.extend(format_spans_cell(
            actions_col4,
            widths[5],
            CellAlign::Center,
        ));

        let actions = Line::from(actions_spans);
        lines.push(clip_line(actions, content_width));
    }

    lines
}

fn format_spans_cell(
    mut spans: Vec<Span<'static>>,
    width: usize,
    align: CellAlign,
) -> Vec<Span<'static>> {
    let content_len: usize = spans.iter().map(|s| s.content.chars().count()).sum();

    if content_len >= width {
        return spans;
    }

    let padding = width - content_len;
    match align {
        CellAlign::Left => {
            spans.push(Span::raw(format!("{:padding$}", "", padding = padding)));
            spans
        }
        CellAlign::Center => {
            let left = padding / 2;
            let right = padding - left;
            let mut result = vec![Span::raw(format!("{:left$}", "", left = left))];
            result.extend(spans);
            result.push(Span::raw(format!("{:right$}", "", right = right)));
            result
        }
    }
}

fn unit_row_column_widths(total_width: u16) -> [usize; 6] {
    let columns = Layout::horizontal([
        UNIT_COLUMN_CONSTRAINTS[0],
        UNIT_COLUMN_CONSTRAINTS[1],
        UNIT_COLUMN_CONSTRAINTS[2],
        UNIT_COLUMN_CONSTRAINTS[3],
        UNIT_COLUMN_CONSTRAINTS[4],
        UNIT_COLUMN_CONSTRAINTS[5],
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
        columns[5].width as usize,
    ]
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

    use crate::models::{UnitInfo, UnitType};

    fn unit() -> UnitInfo {
        UnitInfo {
            name: "ssh.service".to_string(),
            description: "Secure Shell".to_string(),
            scope: crate::models::UnitScope::Global,
            load_state: crate::models::UnitLoadState::Loaded,
            active_state: crate::models::UnitActiveState::Active,
            enablement_state: crate::models::UnitEnablementState::Enabled,
            sub_state: "running".to_string(),
            path: OwnedObjectPath::try_from("/test/unit/ssh").unwrap(),
            fragment_path: "/etc/systemd/system/ssh.service".to_string(),
        }
    }

    #[test]
    fn selected_row_gets_action_line_with_detail_prefix() {
        let lines = format_unit_row(&unit(), &[10, 10, 10, 10, 10, 10], 80, true);

        assert_eq!(lines.len(), 3);
        assert!(lines[0].spans[1].content.contains("service"));
        assert!(lines[1].spans[0].content.starts_with(" ╟ "));
        assert!(
            lines[1]
                .spans
                .iter()
                .any(|span| span.content.contains("Secure Shell"))
        );
        assert!(
            lines[1]
                .spans
                .iter()
                .any(|span| span.content.contains("/etc/systemd/system/ssh.service"))
        );
        assert!(lines[2].spans[0].content.starts_with(" ╙ "));
        assert!(lines[2].spans.iter().any(|span| span.content == "Action: "));
    }

    #[test]
    fn format_unit_row_uses_unknown_type_for_unrecognized_suffix() {
        let mut unknown = unit();
        unknown.name = "ssh.whatever".to_string();

        let lines = format_unit_row(&unknown, &[10, 10, 10, 10, 10, 10], 80, false);

        assert_eq!(lines.len(), 1);
        assert!(
            lines[0]
                .spans
                .iter()
                .any(|span| span.content.contains("unknown"))
        );
        assert_eq!(UnitType::from_unit_name(&unknown.name), UnitType::Unknown);
    }
}
