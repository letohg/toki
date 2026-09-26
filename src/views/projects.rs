use crossterm::event::KeyEvent;
use ratatui::style;
use ratatui::widgets::{Cell, Row, Table};
use ratatui::{Frame, layout::Rect};
use ratatui::{layout::Constraint, style::Style};
use std::str::FromStr;

use crate::database;

pub struct Project {
    projects: Vec<database::Project>,
}

impl Project {
    pub fn new() -> Self {
        Self {
            projects: Vec::new(),
        }
    }

    pub fn refresh(&mut self, projects: Vec<database::Project>) {
        self.projects = projects;
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let rows = self.projects.iter().map(|p| {
            Row::new(vec![
                Cell::from("*").style(Style::new().fg(style::Color::from_str(&p.color).unwrap())),
                Cell::from(p.name.clone()),
                Cell::from(p.description.clone()),
            ])
        });
        let column_widths = vec![
            Constraint::Length(1),
            Constraint::Length(15),
            Constraint::Min(0),
        ];
        let table = Table::new(rows, column_widths);
        frame.render_widget(table, area);
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> bool {
        false
    }
}
