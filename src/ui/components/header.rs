use ratatui::{
    Frame,
    layout::{Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use crate::app::state::{App, FilterMenu};

pub fn draw_unit_header(frame: &mut Frame, app: &App, area: Rect) -> (Rect, Rect, Rect, Rect) {
    let header_layout = Layout::horizontal([
        Constraint::Percentage(30),
        Constraint::Percentage(15),
        Constraint::Percentage(18),
        Constraint::Percentage(18),
        Constraint::Percentage(19),
    ])
    .split(area);

    let search_style = if app.is_searching {
        Style::default().fg(Color::Yellow)
    } else if !app.search_query.is_empty() {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    let search_text = if app.search_query.is_empty() && !app.is_searching {
        Text::from("Type / to search...")
    } else {
        Text::from(app.search_query.as_str())
    };
    frame.render_widget(
        Paragraph::new(search_text).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Search (/) ")
                .border_style(search_style),
        ),
        header_layout[0],
    );

    draw_filter_segment(frame, app, header_layout[1], FilterMenu::Scope);
    draw_filter_segment(frame, app, header_layout[2], FilterMenu::Active);
    draw_filter_segment(frame, app, header_layout[3], FilterMenu::Enablement);
    draw_filter_segment(frame, app, header_layout[4], FilterMenu::Load);

    (
        header_layout[1],
        header_layout[2],
        header_layout[3],
        header_layout[4],
    )
}

fn draw_filter_segment(frame: &mut Frame, app: &App, area: Rect, menu: FilterMenu) {
    let border_style = if app.open_filter_menu == Some(menu) {
        Style::default().fg(Color::Yellow)
    } else if app.filter_summary(menu) != "all" {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    let value_style = if app.filter_summary(menu) == "all" {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    };

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            app.filter_summary(menu),
            value_style,
        )))
        .centered()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(menu.segment_title())
                .border_style(border_style),
        ),
        area,
    );
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
            let marker = if option.selected { "(*)" } else { "( )" };
            let style = if option.selected {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
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

use ratatui::layout::Constraint;
