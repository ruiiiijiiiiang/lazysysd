use ansi_to_tui::IntoText;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::{
    app::auth::EmbeddedAuthFlow,
    models::EditReview,
    ui::utils::{
        AUTH_MODAL_HEIGHT, AUTH_MODAL_WIDTH, EDIT_REVIEW_MODAL_HEIGHT, EDIT_REVIEW_MODAL_WIDTH,
        keybind_style, modal_border_style,
    },
};

pub fn render_edit_review_modal(frame: &mut Frame, review: &EditReview) {
    let area = centered_rect(
        EDIT_REVIEW_MODAL_WIDTH,
        EDIT_REVIEW_MODAL_HEIGHT,
        frame.area(),
    );
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title(format!(" Apply {} ", review.mode.action_label()))
        .borders(Borders::ALL)
        .border_style(modal_border_style());
    frame.render_widget(block, area);

    let inner = area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let layout = Layout::vertical([Constraint::Min(4), Constraint::Length(3)]).split(inner);

    let body = vec![
        Line::from(format!("Unit: {}", review.unit_name)),
        Line::from(format!(
            "Mode: {} via systemctl edit",
            review.mode.action_label()
        )),
        Line::from("Draft returned from your editor and is ready to apply."),
        Line::from("Applying will request authorization and reload systemd automatically."),
    ];
    frame.render_widget(
        Paragraph::new(body)
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title(" Draft ")),
        layout[0],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Enter", keybind_style()),
            Span::raw(": apply    "),
            Span::styled("Esc/q", keybind_style()),
            Span::raw(": discard"),
        ]))
        .centered()
        .block(Block::default().borders(Borders::ALL)),
        layout[1],
    );
}

pub fn render_error_modal(frame: &mut Frame, message: &str) {
    let area = centered_rect(60, 20, frame.area());
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title(" Error ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));
    frame.render_widget(block, area);

    let inner = area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let layout = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner);

    frame.render_widget(
        Paragraph::new(message)
            .wrap(Wrap { trim: false })
            .alignment(ratatui::layout::Alignment::Center),
        layout[0],
    );

    frame.render_widget(
        Paragraph::new("Press any key to dismiss")
            .centered()
            .style(Style::default().fg(Color::DarkGray)),
        layout[1],
    );
}

pub fn render_auth_modal(frame: &mut Frame, auth: &EmbeddedAuthFlow) {
    let area = centered_rect(AUTH_MODAL_WIDTH, AUTH_MODAL_HEIGHT, frame.area());
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title(" Authentication Required ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    frame.render_widget(block, area);

    let inner = area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });

    let prompt = if auth.pane.output.trim().is_empty() {
        Text::from("Waiting for polkit agent...")
    } else {
        match auth.pane.output.as_bytes().into_text() {
            Ok(t) => t,
            Err(_) => Text::from(auth.pane.output.as_str()),
        }
    };
    frame.render_widget(Paragraph::new(prompt).wrap(Wrap { trim: false }), inner);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}
