use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    widgets::{Block, Borders, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

use crate::{
    app::state::context::{App, FilterMenu, ViewMode},
    ui::components::{
        file_view::draw_file_view, header::draw_unit_header, header::render_filter_menu,
        help::draw_help, log_view::draw_log_view, modals::render_auth_modal,
        modals::render_edit_review_modal, modals::render_error_modal, unit_list::draw_unit_list,
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
    let area = frame.area();
    app.terminal_size = (area.width, area.height);

    let main_layout = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(10),
        Constraint::Length(6),
    ])
    .split(frame.area());

    app.main_content_height = main_layout[1].height.saturating_sub(2);

    let filter_anchors = Some(draw_unit_header(frame, app, main_layout[0]));

    match app.view_mode {
        ViewMode::UnitList => draw_unit_list(frame, app, main_layout[1]),
        ViewMode::LogView => draw_log_view(frame, app, main_layout[1]),
        ViewMode::FileView => draw_file_view(frame, app, main_layout[1]),
    }

    let help_block = Block::default().borders(Borders::ALL).title(" Help ");
    frame.render_widget(help_block, main_layout[2]);
    draw_help(
        frame,
        app,
        main_layout[2].inner(Margin {
            vertical: 1,
            horizontal: 1,
        }),
    );

    if let Some((type_rect, scope_rect, active_rect, enablement_rect, load_rect)) = filter_anchors
        && let Some(menu) = app.unit_list.open_filter_menu
    {
        let anchor = match menu {
            FilterMenu::Type => type_rect,
            FilterMenu::Scope => scope_rect,
            FilterMenu::Active => active_rect,
            FilterMenu::Enablement => enablement_rect,
            FilterMenu::Load => load_rect,
        };
        render_filter_menu(frame, app, menu, anchor, main_layout[1]);
    }

    if let Some(review) = &app.pending_edit_review {
        render_edit_review_modal(frame, review);
    }

    if let Some(auth) = &app.embedded_auth {
        render_auth_modal(frame, auth);
    }

    if let Some(error) = &app.error_message {
        render_error_modal(frame, error);
    }
}
