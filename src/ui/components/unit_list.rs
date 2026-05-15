use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::{app::state::App, models::UnitInfo, ui::render::render_scrollbar};

pub fn draw_unit_list(frame: &mut Frame, app: &mut App, area: Rect) {
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

fn format_unit_row(unit: &UnitInfo, widths: &[usize; 5]) -> Line<'static> {
    let active_sub = format!("{} ({})", unit.active_state, unit.sub_state);
    Line::from(vec![
        Span::styled(
            format_cell(&unit.name, widths[0], CellAlign::Left),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format_cell(&unit.scope, widths[1], CellAlign::Center),
            Style::default().fg(Color::DarkGray),
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
    ])
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
