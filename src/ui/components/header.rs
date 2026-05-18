use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use crate::{
    app::state::context::{App, FilterMenu, ViewMode},
    models::UnitType,
    ui::utils::{
        UNIT_COLUMN_CONSTRAINTS, keybind_style, modal_border_style, search_cursor_style,
        search_query_style, selection_style,
    },
};

pub fn draw_unit_header(
    frame: &mut Frame,
    app: &App,
    area: Rect,
) -> (Rect, Rect, Rect, Rect, Rect) {
    let header_layout = Layout::horizontal([
        Constraint::Percentage(30),
        UNIT_COLUMN_CONSTRAINTS[1],
        UNIT_COLUMN_CONSTRAINTS[2],
        UNIT_COLUMN_CONSTRAINTS[3],
        UNIT_COLUMN_CONSTRAINTS[4],
        UNIT_COLUMN_CONSTRAINTS[5],
    ])
    .split(area);

    let search_area = header_layout[0];
    let type_area = header_layout[1];
    let scope_area = header_layout[2];
    let active_area = header_layout[3];
    let enablement_area = header_layout[4];
    let load_area = header_layout[5];

    draw_search_segment(frame, app, search_area);
    draw_status_segment(frame, app, type_area, FilterMenu::Type);
    draw_status_segment(frame, app, scope_area, FilterMenu::Scope);
    draw_status_segment(frame, app, active_area, FilterMenu::Active);
    draw_status_segment(frame, app, enablement_area, FilterMenu::Enablement);
    draw_status_segment(frame, app, load_area, FilterMenu::Load);

    (
        type_area,
        scope_area,
        active_area,
        enablement_area,
        load_area,
    )
}

fn draw_search_segment(frame: &mut Frame, app: &App, area: Rect) {
    let (text, style) = search_segment_content(app);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Line::from(vec![
            Span::raw(" Search ["),
            Span::styled("/", keybind_style()),
            Span::raw("] "),
        ]))
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
        (Text::from(placeholder), search_segment_style(false, false))
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
            spans.push(Span::styled(ch.to_string(), search_cursor_style()));
        } else {
            spans.push(Span::raw(ch.to_string()));
        }
    }

    if cursor == query.chars().count() {
        spans.push(Span::styled(" ", search_cursor_style()));
    }

    Text::from(Line::from(spans))
}

fn search_segment_style(searching: bool, has_query: bool) -> Style {
    if searching {
        Style::default().fg(Color::Yellow)
    } else if has_query {
        search_query_style()
    } else {
        Style::default()
    }
}

fn draw_status_segment(frame: &mut Frame, app: &App, area: Rect, menu: FilterMenu) {
    let spec = status_segment_spec(app, menu);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(filter_segment_title(
            menu,
            app.view_mode == ViewMode::UnitList,
        ))
        .border_style(spec.border_style);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(spec.value, spec.value_style)))
            .centered()
            .block(block),
        area,
    );
}

fn filter_segment_title(menu: FilterMenu, show_hotkey: bool) -> Line<'static> {
    let label = match menu {
        FilterMenu::Type => " Type ",
        FilterMenu::Scope => " Scope ",
        FilterMenu::Active => " Active ",
        FilterMenu::Enablement => " Enablement ",
        FilterMenu::Load => " Load ",
    };
    let hotkey = match menu {
        FilterMenu::Type => 'y',
        FilterMenu::Scope => 'p',
        FilterMenu::Active => 'a',
        FilterMenu::Enablement => 'n',
        FilterMenu::Load => 'o',
    };

    if show_hotkey {
        Line::from(vec![
            Span::raw(label.trim_end()),
            Span::raw(" ["),
            Span::styled(hotkey.to_string(), keybind_style()),
            Span::raw("] "),
        ])
    } else {
        Line::from(label)
    }
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
    let value = app.filter_summary(menu);
    let value_style = if value == "all" {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White).bold()
    };
    let border_style = if app.unit_list.open_filter_menu == Some(menu) {
        Style::default().fg(Color::Yellow)
    } else if value != "all" {
        modal_border_style()
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

    let unit_type = UnitType::from_unit_name(&unit.name);

    let (value, value_style) = match menu {
        FilterMenu::Type => (
            unit_type.as_str().to_string(),
            Style::default().fg(unit_type.color()),
        ),
        FilterMenu::Scope => (
            unit.scope.as_str().to_string(),
            Style::default().fg(unit.scope.color()),
        ),
        FilterMenu::Active => (
            format!("{} ({})", unit.active_state.as_str(), unit.sub_state),
            Style::default().fg(unit.active_state.color()),
        ),
        FilterMenu::Enablement => (
            unit.enablement_state.as_str().to_string(),
            Style::default().fg(unit.enablement_state.color()),
        ),
        FilterMenu::Load => (
            unit.load_state.as_str().to_string(),
            Style::default().fg(unit.load_state.color()),
        ),
    };

    SegmentSpec {
        value,
        value_style,
        border_style: Style::default().fg(Color::DarkGray),
    }
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
                selection_style()
            } else {
                Style::default()
            };
            let label_with_count = format!("{} ({})", option.label, option.count);
            ListItem::new(Line::from(vec![
                Span::styled(format!("{marker} ["), style),
                Span::styled(option.hotkey.to_string(), style.patch(keybind_style())),
                Span::styled("] ", style),
                Span::styled(label_with_count, style),
            ]))
        })
        .collect();

    frame.render_widget(Clear, area);
    frame.render_widget(
        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(filter_segment_title(menu, true))
                .border_style(Style::default().fg(Color::Yellow)),
        ),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;
    use zbus::zvariant::OwnedObjectPath;

    use crate::{
        app::state::context::App,
        models::{UnitActiveState, UnitEnablementState, UnitInfo, UnitLoadState, UnitScope},
    };

    fn test_app(unit: Option<UnitInfo>) -> App {
        let (tx, _rx) = mpsc::channel(1);
        let mut app = App::blank(tx);
        if let Some(unit) = unit {
            app.unit_list.units = vec![unit];
            app.unit_list.filtered_indices = vec![0];
            app.unit_list.select_index(Some(0));
        }
        app.is_loading = false;
        app
    }

    fn unit(name: &str) -> UnitInfo {
        UnitInfo {
            name: name.to_string(),
            description: "Secure Shell".to_string(),
            scope: UnitScope::Session,
            load_state: UnitLoadState::Loaded,
            active_state: UnitActiveState::Active,
            enablement_state: UnitEnablementState::Enabled,
            sub_state: "running".to_string(),
            path: OwnedObjectPath::try_from("/test/unit/ssh").unwrap(),
            fragment_path: format!("/etc/systemd/system/{name}"),
        }
    }

    #[test]
    fn selected_unit_segment_spec_uses_selected_unit_values() {
        let app = test_app(Some(unit("ssh.socket")));

        let spec = selected_unit_segment_spec(&app, FilterMenu::Type);

        assert_eq!(spec.value, "socket");
        assert_eq!(spec.value_style.fg, Some(Color::Cyan));
        assert_eq!(spec.border_style.fg, Some(Color::DarkGray));
    }

    #[test]
    fn selected_unit_segment_spec_returns_unknown_when_no_unit_selected() {
        let app = test_app(None);

        let spec = selected_unit_segment_spec(&app, FilterMenu::Load);

        assert_eq!(spec.value, "Unknown");
        assert_eq!(spec.value_style.fg, Some(Color::DarkGray));
        assert_eq!(spec.border_style.fg, Some(Color::DarkGray));
    }
}
