use crossterm::event::KeyEvent;
use ratatui::layout::Layout;
use ratatui::style::{self, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Cell, Padding, Paragraph, Row, Table};
use ratatui::{Frame, layout::Rect};
use ratatui::{layout::Constraint, style::Style};
use std::collections::HashMap;
use std::default;
use std::str::FromStr;

use crate::database::{self, EntityId};
use crate::pages::format_duration_hms_verbose;

pub struct Project {
    projects: Vec<database::Project>,
    metrics_per_project: HashMap<EntityId, database::IntervalMetrics>,
}

impl Project {
    pub fn new() -> Self {
        Self {
            projects: Vec::new(),
            metrics_per_project: HashMap::new(),
        }
    }

    pub fn refresh(
        &mut self,
        projects: Vec<database::Project>,
        metrics_per_project: HashMap<EntityId, database::IntervalMetrics>,
    ) {
        self.projects = projects;
        self.metrics_per_project = metrics_per_project;
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
            let metrics = self.metrics_per_project.get(&p.id);
            Row::new(vec![
                Cell::from(p.name.clone())
                    .style(Style::new().fg(style::Color::from_str(&p.color).unwrap())),
                Cell::from(p.description.clone()),
                Cell::from(metrics.map_or("--:--".to_string(), |m| {
                    format_duration_hms_verbose(&m.total_time)
                })),
                Cell::from(
                    Line::from(metrics.map_or("in 0 sessions".to_string(), |m| {
                        format!("in {:2} sessions", m.session_count)
                    }))
                    .right_aligned(),
                ),
            ])
        });
        let column_widths = vec![
            Constraint::Length(15),
            Constraint::Min(7),
            Constraint::Length(10),
            Constraint::Length(22),
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
