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
    Info(String, Timestamp),
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

    pub fn notify_info(&mut self, msg: &str) {
        self.notificatinos
            .push(Notification::Info(msg.to_string(), Timezone::now()));
    }

    pub fn notify_error(&mut self, msg: &str) {
        self.notificatinos
            .push(Notification::Error(msg.to_string(), Timezone::now()));
    }

    pub fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        if self.notificatinos.is_empty() {
            return;
        }

        let now = Timezone::now();
        self.notificatinos.retain(|n| {
            match &n {
                Notification::Info(_, creation_time) | Notification::Error(_, creation_time) => {
                    if now - creation_time > chrono::Duration::seconds(3) {
                        return false;
                    }
                }
            }
            true
        });
        if self.notificatinos.is_empty() {
            return;
        }

        self.notificatinos.iter().enumerate().for_each(|(i, n)| {
            if (i * 5) > (area.height - 1) as usize {
                return;
                // return Err(());
            }
            let notification_area = Rect {
                y: area.height - 1 - (i * 5) as u16,
                x: area.width - 17,
                height: 5,
                width: 20,
            };
            let notification_text_area = Rect {
                y: notification_area.y + 2,
                x: notification_area.x + 1,
                width: notification_area.width - 2,
                height: 3,
            };
            frame.render_widget(Clear, notification_area);

            let (category, message, color, time) = match &n {
                Notification::Info(msg, time) => ("Info", msg.clone(), theme::GREEN, time),
                Notification::Error(msg, time) => ("Error", msg.clone(), theme::RED, time),
            };
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
            // Ok(())
        });
    }
}
