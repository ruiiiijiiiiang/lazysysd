use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    widgets::{Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

use crate::{
    app::state::{App, ViewMode},
    ui::components::{
        file_view::draw_file_view,
        header::draw_unit_header,
        header::render_filter_menu,
        help::help_text,
        logs::draw_log_view,
        modals::render_auth_modal,
        modals::render_edit_review_modal,
        unit_list::draw_unit_list,
    },
};

pub fn render_scrollbar(frame: &mut Frame, area: Rect, position: usize, content_length: usize) {
    if content_length <= area.height as usize {
        return;
    }

    let scrollbar = Scrollbar::default()
        .orientation(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"));

    let mut scrollbar_state =
        ScrollbarState::new(content_length.saturating_sub(area.height as usize)).position(position);

    frame.render_stateful_widget(
        scrollbar,
        area.inner(Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut scrollbar_state,
    );
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let main_layout = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(10),
        Constraint::Length(8),
    ])
    .split(frame.area());

    let mut filter_anchors = None;

    if app.view_mode == ViewMode::UnitList {
        filter_anchors = Some(draw_unit_header(frame, app, main_layout[0]));
    } else {
        let title = match app.view_mode {
            ViewMode::LogView => "Logs",
            ViewMode::FileView => "Unit File",
            ViewMode::UnitList => "",
        };
        let unit_name = app
            .list_state
            .selected()
            .and_then(|i| app.units.get(app.filtered_units[i]))
            .map(|u| u.name.as_str())
            .unwrap_or("Unknown");
        let header = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {}: {} (Esc to back) ", title, unit_name));
        frame.render_widget(
            Paragraph::new("Press Esc to return to the unit list").block(header),
            main_layout[0],
        );
    }

    match app.view_mode {
        ViewMode::UnitList => draw_unit_list(frame, app, main_layout[1]),
        ViewMode::LogView => draw_log_view(frame, app, main_layout[1]),
        ViewMode::FileView => draw_file_view(frame, app, main_layout[1]),
    }

    let help_layout = Layout::horizontal([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(main_layout[2]);

    let logs_items: Vec<ListItem> = app
        .logs
        .iter()
        .rev()
        .map(|s| ListItem::new(s.as_str()))
        .collect();
    frame.render_widget(
        List::new(logs_items).block(Block::default().borders(Borders::ALL).title(" Event Log ")),
        help_layout[0],
    );

    frame.render_widget(
        Paragraph::new(help_text(app))
            .block(Block::default().borders(Borders::ALL).title(" Help ")),
        help_layout[1],
    );

    if let Some((scope_rect, active_rect, enablement_rect, load_rect)) = filter_anchors
        && let Some(menu) = app.open_filter_menu
    {
        let anchor = match menu {
            crate::app::state::FilterMenu::Scope => scope_rect,
            crate::app::state::FilterMenu::Active => active_rect,
            crate::app::state::FilterMenu::Enablement => enablement_rect,
            crate::app::state::FilterMenu::Load => load_rect,
        };
        render_filter_menu(frame, app, menu, anchor, main_layout[1]);
    }

    if let Some(review) = &app.pending_edit_review {
        render_edit_review_modal(frame, review);
    }

    if let Some(auth) = &app.embedded_auth {
        render_auth_modal(frame, auth);
    }
}
