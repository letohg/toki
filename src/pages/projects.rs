use crossterm::event::KeyEvent;
use ratatui::layout::Layout;
use ratatui::style::{self, Stylize};
use ratatui::widgets::{Block, Borders, Cell, Padding, Paragraph, Row, Table};
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
        let block = Block::new().padding(Padding::symmetric(1, 1));
        let [title_area, entries_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(3)]).areas(block.inner(area));
        frame.render_widget(Paragraph::new("Projects"), title_area);
        let table_block = Block::new().borders(Borders::TOP);
        frame.render_widget(&table_block, entries_area);
        let table_area = table_block.inner(entries_area);

        let headings = Row::new(
            ["Name", "Description", "Total Hours"]
                .into_iter()
                .map(|t| Cell::from(t).bold()),
        );
        let rows = self.projects.iter().map(|p| {
            Row::new(vec![
                Cell::from(p.name.clone())
                    .style(Style::new().fg(style::Color::from_str(&p.color).unwrap())),
                Cell::from(p.description.clone()),
                "--:--".into(),
            ])
        });
        let column_widths = vec![
            Constraint::Length(15),
            Constraint::Min(7),
            Constraint::Length(10),
        ];
        let table = Table::new(rows, column_widths)
            .header(headings)
            .style(Style::new().add_modifier(style::Modifier::BOLD));
        frame.render_widget(table, table_area);
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> bool {
        false
    }
}
