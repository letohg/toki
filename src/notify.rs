use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{self, Color, Stylize},
    text::Line,
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::{
    database::{Timestamp, Timezone},
    pages::{format_duration_clock_hm, format_timestamp},
    theme,
};

pub enum Notification {
    Success(String, Timestamp),
    Info(String, Timestamp),
    Warning(String, Timestamp),
    Error(String, Timestamp),
}

pub struct Notifier {
    notificatinos: Vec<Notification>,
}

impl Notifier {
    pub fn new() -> Self {
        Self {
            notificatinos: Vec::new(),
        }
    }

    pub fn notify_success(&mut self, msg: &str) {
        self.notificatinos
            .push(Notification::Success(msg.to_string(), Timezone::now()));
    }

    pub fn notify_info(&mut self, msg: &str) {
        self.notificatinos
            .push(Notification::Info(msg.to_string(), Timezone::now()));
    }

    pub fn notify_warning(&mut self, msg: &str) {
        self.notificatinos
            .push(Notification::Warning(msg.to_string(), Timezone::now()));
    }

    pub fn notify_error(&mut self, msg: &str) {
        self.notificatinos
            .push(Notification::Error(msg.to_string(), Timezone::now()));
    }

    pub fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        self.remove_outdated_notifications();
        if self.notificatinos.is_empty() {
            return;
        }

        self.notificatinos.iter().enumerate().for_each(|(i, n)| {
            let box_height = 3;
            if (i * box_height) > (area.height - 1) as usize {
                return;
            }
            let (icon, category, message, color, time) = match &n {
                Notification::Success(msg, time) => (" ", "Info", msg.clone(), theme::GREEN, time),
                Notification::Info(msg, time) => (" ", "Info", msg.clone(), theme::GREEN, time),
                Notification::Warning(msg, time) => {
                    (" ", "Warning", msg.clone(), theme::YELLOW, time)
                }
                Notification::Error(msg, time) => ("", "Error", msg.clone(), theme::RED, time),
            };

            let box_width = (4 + 2 + message.len()) as u16;

            let notification_area = Rect {
                y: area.height + 1 - (i * box_height) as u16,
                x: area.width - box_width + 4,
                height: box_height as u16,
                width: box_width,
            };
            let notification_text_area = Rect {
                y: notification_area.y + 2,
                x: notification_area.x + 1,
                width: notification_area.width - 2,
                height: 3,
            };
            frame.render_widget(Clear, notification_area);
            let message_block = Block::bordered()
                .bg(theme::BASE)
                .bold()
                .border_style(style::Style::new().fg(color).bold())
                .border_type(BorderType::Rounded);

            let message_text = Paragraph::new(format!("{icon} {message}"))
                .fg(color)
                .bg(theme::BASE)
                .bold()
                .block(message_block);
            frame.render_widget(&message_text, notification_area);

            // render_notification(
            //     frame,
            //     notification_area,
            //     notification_text_area,
            //     category,
            //     message,
            //     color,
            //     time,
            // );
        });
    }

    fn remove_outdated_notifications(&mut self) {
        let now = Timezone::now();
        self.notificatinos.retain(|n| {
            match &n {
                Notification::Success(_, creation_time)
                | Notification::Info(_, creation_time)
                | Notification::Warning(_, creation_time)
                | Notification::Error(_, creation_time) => {
                    if now - creation_time > chrono::Duration::seconds(3) {
                        return false;
                    }
                }
            }
            true
        });
    }
}

fn render_notification(
    frame: &mut Frame<'_>,
    notification_area: Rect,
    notification_text_area: Rect,
    category: &str,
    message: String,
    color: Color,
    time: &chrono::prelude::DateTime<chrono::prelude::Utc>,
) {
    let notification_block = Block::bordered()
        .bg(theme::BASE)
        .bold()
        .border_type(BorderType::Rounded)
        .border_style(style::Style::new().fg(color).bold());

    let message_block = Block::bordered()
        .bg(theme::BASE)
        .bold()
        .border_style(style::Style::new().fg(color).bold())
        .borders(Borders::TOP);

    let notification = Paragraph::new(category).fg(color);
    frame.render_widget(&notification, notification_block.inner(notification_area));
    let time = Paragraph::new(time.format("%H:%M:%S").to_string())
        .right_aligned()
        .italic()
        .fg(color)
        .block(notification_block);
    let message_text = Paragraph::new(message)
        .fg(color)
        .bg(theme::BASE)
        .bold()
        .block(message_block);
    frame.render_widget(&time, notification_area);
    frame.render_widget(&message_text, notification_text_area);
}
