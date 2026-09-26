use ratatui::style;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    widgets::{Cell, Paragraph, Row, Table},
};
use std::str::FromStr;

use crate::database;
use crate::views::delta_hms;
use crate::views::format_timestamp;

pub struct Timer {
    active_entry: Option<database::TimeEntry>,
    recent_entries: Vec<database::TimeEntry>,
    projects: Vec<database::Project>,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            active_entry: None,
            recent_entries: Vec::new(),
            projects: Vec::new(),
        }
    }

    pub fn refresh(
        &mut self,
        active_entry: Option<database::TimeEntry>,
        recent_entries: Vec<database::TimeEntry>,
        projects: Vec<database::Project>,
    ) {
        self.active_entry = active_entry;
        self.recent_entries = recent_entries;
        self.projects = projects;
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let [tracker_area, entries_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
        let tracker_text = if let Some(entry) = &self.active_entry {
            format!(
                "Tracking: {} | {} | {}",
                entry.project_id,
                entry.description,
                delta_hms(&entry.start_time, &entry.end_time)
            )
        } else {
            "No running entry".to_string()
        };

        let p = Paragraph::new(tracker_text)
            .style(style::Color::Yellow)
            .left_aligned();
        frame.render_widget(p, tracker_area);

        let rows = self.recent_entries.iter().map(|e| {
            let project = self.projects.iter().find(|p| p.id == e.project_id);
            let project_color = project.map_or(style::Color::Yellow, |p| {
                style::Color::from_str(&p.color).unwrap()
            });
            Row::new(vec![
                Cell::from("*").style(style::Style::new().fg(project_color)),
                Cell::from(project.map_or("None".to_string(), |p| p.name.clone())),
                Cell::from(e.description.clone()),
                Cell::from(format!(
                    "{} - {}",
                    format_timestamp(&e.start_time),
                    e.end_time
                        .map_or("Running".to_owned(), |ts| format_timestamp(&ts))
                )),
                Cell::from(delta_hms(&e.start_time, &e.end_time)),
            ])
        });
        let column_widths = vec![
            Constraint::Length(1),
            Constraint::Length(15),
            Constraint::Min(0),
            Constraint::Length(16), // start time - end time
            Constraint::Length(8),  // duration
        ];
        let table = Table::new(rows, column_widths);
        frame.render_widget(table, entries_area);
    }
}
