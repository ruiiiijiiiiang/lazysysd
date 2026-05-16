use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use crate::app::state::{App, FilterMenu, ViewMode};

pub fn draw_unit_header(frame: &mut Frame, app: &App, area: Rect) -> (Rect, Rect, Rect, Rect) {
    let header_layout = Layout::horizontal([
        Constraint::Percentage(30),
        Constraint::Percentage(15),
        Constraint::Percentage(18),
        Constraint::Percentage(18),
        Constraint::Percentage(19),
    ])
    .split(area);

    let search_area = header_layout[0];
    let scope_area = header_layout[1];
    let active_area = header_layout[2];
    let enablement_area = header_layout[3];
    let load_area = header_layout[4];

    draw_search_segment(frame, app, search_area);
    draw_status_segment(frame, app, scope_area, FilterMenu::Scope);
    draw_status_segment(frame, app, active_area, FilterMenu::Active);
    draw_status_segment(frame, app, enablement_area, FilterMenu::Enablement);
    draw_status_segment(frame, app, load_area, FilterMenu::Load);

    (scope_area, active_area, enablement_area, load_area)
}

fn draw_search_segment(frame: &mut Frame, app: &App, area: Rect) {
    let (text, style) = search_segment_content(app);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Search (/) ")
        .border_style(style);

    frame.render_widget(Paragraph::new(text).block(block), area);
}

fn search_segment_content(app: &App) -> (Text<'static>, Style) {
    let placeholder = match app.view_mode {
        ViewMode::UnitList => "Type / to search units...",
        ViewMode::LogView => "Type / to search logs...",
        ViewMode::FileView => "Type / to search unit file...",
    };

    if app.search.query.is_empty() && !app.search.is_active {
        (
            Text::from(placeholder),
            search_segment_style(false, false),
        )
    } else {
        (
            render_search_text(&app.search.query, app.search.cursor),
            search_segment_style(app.search.is_active, !app.search.query.is_empty()),
        )
    }
}

fn render_search_text(query: &str, cursor: usize) -> Text<'static> {
    let chars: Vec<char> = query.chars().collect();
    let cursor = cursor.min(chars.len());
    let mut spans = Vec::with_capacity(chars.len());

    for (index, ch) in chars.into_iter().enumerate() {
        if index == cursor {
            spans.push(Span::styled(
                ch.to_string(),
                Style::default().bg(Color::Yellow).fg(Color::Black).bold(),
            ));
        } else {
            spans.push(Span::raw(ch.to_string()));
        }
    }

    if cursor == query.chars().count() {
        spans.push(Span::styled(
            " ",
            Style::default().bg(Color::Yellow).fg(Color::Black).bold(),
        ));
    }

    Text::from(Line::from(spans))
}

fn search_segment_style(searching: bool, has_query: bool) -> Style {
    if searching {
        Style::default().fg(Color::Yellow)
    } else if has_query {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    }
}

fn draw_status_segment(frame: &mut Frame, app: &App, area: Rect, menu: FilterMenu) {
    let spec = status_segment_spec(app, menu);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(menu.segment_title(app.view_mode == ViewMode::UnitList))
        .border_style(spec.border_style);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(spec.value, spec.value_style)))
            .centered()
            .block(block),
        area,
    );
}

struct SegmentSpec {
    value: String,
    value_style: Style,
    border_style: Style,
}

fn status_segment_spec(app: &App, menu: FilterMenu) -> SegmentSpec {
    match app.view_mode {
        ViewMode::UnitList => unit_list_segment_spec(app, menu),
        ViewMode::LogView | ViewMode::FileView => selected_unit_segment_spec(app, menu),
    }
}

fn unit_list_segment_spec(app: &App, menu: FilterMenu) -> SegmentSpec {
    let value = app.filter_summary(menu).to_string();
    let value_style = if value == "all" {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White).bold()
    };
    let border_style = if app.unit_list.open_filter_menu == Some(menu) {
        Style::default().fg(Color::Yellow)
    } else if value != "all" {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    SegmentSpec {
        value,
        value_style,
        border_style,
    }
}

fn selected_unit_segment_spec(app: &App, menu: FilterMenu) -> SegmentSpec {
    let Some(unit) = app.get_selected_unit() else {
        return SegmentSpec {
            value: "Unknown".to_string(),
            value_style: Style::default().fg(Color::DarkGray),
            border_style: Style::default().fg(Color::DarkGray),
        };
    };

    let (value, value_style) = match menu {
        FilterMenu::Scope => (unit.scope.clone(), Style::default().fg(Color::DarkGray)),
        FilterMenu::Active => (
            format!("{} ({})", unit.active_state, unit.sub_state),
            active_state_style(&unit.active_state),
        ),
        FilterMenu::Enablement => (
            unit.enablement_state.clone(),
            enablement_state_style(&unit.enablement_state),
        ),
        FilterMenu::Load => (unit.load_state.clone(), load_state_style(&unit.load_state)),
    };

    SegmentSpec {
        value,
        value_style,
        border_style: Style::default().fg(Color::DarkGray),
    }
}

fn active_state_style(state: &str) -> Style {
    Style::default().fg(match state {
        "active" => Color::Green,
        "failed" => Color::Red,
        "inactive" => Color::DarkGray,
        "activating" | "reloading" => Color::Yellow,
        "deactivating" => Color::LightYellow,
        "maintenance" => Color::Magenta,
        _ => Color::White,
    })
}

fn enablement_state_style(state: &str) -> Style {
    Style::default().fg(match state {
        "enabled" | "enabled-runtime" => Color::Green,
        "static" | "generated" | "alias" | "indirect" | "linked" | "linked-runtime" => Color::Cyan,
        "disabled" | "disabled-runtime" => Color::DarkGray,
        "masked" | "masked-runtime" | "invalid" => Color::Red,
        "transient" | "unknown" => Color::Yellow,
        _ => Color::White,
    })
}

fn load_state_style(state: &str) -> Style {
    Style::default().fg(match state {
        "loaded" => Color::Green,
        "not-found" => Color::Yellow,
        "bad-setting" | "error" | "masked" => Color::Red,
        _ => Color::White,
    })
}

pub fn render_filter_menu(
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
            let marker = if option.selected { "◉" } else { "○" };
            let style = if option.selected {
                Style::default().fg(Color::Green).bold()
            } else {
                Style::default()
            };
            let label_with_count = format!("{} ({})", option.label, option.count);
            ListItem::new(Line::from(vec![
                Span::styled(format!("{marker} [{}] ", option.hotkey), style),
                Span::styled(label_with_count, style),
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
