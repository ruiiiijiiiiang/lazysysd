use ansi_to_tui::IntoText;
use ratatui::{
    Frame,
    layout::{Layout, Margin, Rect},
    style::{Color, Style},
    text::{Line, Text},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::{
    models::EditReview,
    systemd::auth::EmbeddedAuthFlow,
};

pub fn render_edit_review_modal(frame: &mut Frame, review: &EditReview) {
    let area = centered_rect(72, 38, frame.area());
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title(format!(" Apply {} ", review.mode.action_label()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
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
        Paragraph::new("a / Enter: apply    d / Esc: discard")
            .centered()
            .block(Block::default().borders(Borders::ALL)),
        layout[1],
    );
}

pub fn render_auth_modal(frame: &mut Frame, auth: &EmbeddedAuthFlow) {
    let area = centered_rect(80, 60, frame.area());
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
    let layout = Layout::vertical([Constraint::Min(5), Constraint::Length(3)]).split(inner);

    let prompt = if auth.pane.output.trim().is_empty() {
        Text::from("Waiting for polkit agent...")
    } else {
        match auth.pane.output.as_bytes().into_text() {
            Ok(t) => t,
            Err(_) => Text::from(auth.pane.output.as_str()),
        }
    };
    frame.render_widget(
        Paragraph::new(prompt)
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title(" Prompt ")),
        layout[0],
    );
    frame.render_widget(
        Paragraph::new("Enter password into terminal. Esc to cancel.")
            .centered()
            .block(Block::default().borders(Borders::ALL)),
        layout[1],
    );
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

use ratatui::layout::Constraint;
use ratatui::widgets::Wrap;
