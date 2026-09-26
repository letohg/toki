use crossterm::event::{KeyCode, KeyEvent};
use ratatui::style::{self, Color};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    widgets::{Cell, Paragraph, Row, Table},
};
use std::str::FromStr;

use crate::database::{self, EntityId};
use crate::views::delta_hms;
use crate::views::format_timestamp;

pub struct Timer {
    active_entry: Option<database::TimeEntry>,
    recent_entries: Vec<database::TimeEntry>,
    projects: Vec<database::Project>,
}

pub enum TimerAction {
    StartTracking(EntityId, String, Option<EntityId>),
    StopTracking(EntityId),
}

struct TrackingInfo {
    duration: String,
    project_name: String,
    project_color: Color,
    task: String,
    description: String,
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
            Layout::vertical([Constraint::Length(5), Constraint::Min(0)]).areas(area);

        let tracking_info = match &self.active_entry {
            Some(entry) => {
                let project = self.projects.iter().find(|p| p.id == entry.project_id);
                let project_color = project.map_or(style::Color::Yellow, |p| {
                    style::Color::from_str(&p.color).unwrap_or_default()
                });

                let duration = delta_hms(&entry.start_time, &entry.end_time);
                let project_name = project.map_or("None".to_string(), |p| p.name.clone());
                let description = entry.description.clone();
                Some(TrackingInfo {
                    duration,
                    project_name,
                    project_color,
                    task: "-".to_owned(),
                    description,
                })
            }
            None => None,
        };
        render_tracker(frame, tracker_area, tracking_info);

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

    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Option<TimerAction> {
        match key_event.code {
            KeyCode::Char('s') if let Some(entry) = &self.active_entry => {
                Some(TimerAction::StopTracking(entry.id))
            }
            KeyCode::Char('s') => {
                let project_id = 1;
                let description = "Description".to_owned();
                let task_id = None;
                Some(TimerAction::StartTracking(project_id, description, task_id))
            }
            KeyCode::Char('e') => {
                // open edit entry mode
                // Project:     [...]
                // Task:        [...]
                // Description: [...]
                None
            }
            _ => None,
        }
    }
}

fn render_tracker(frame: &mut Frame<'_>, tracker_area: Rect, tracking_info: Option<TrackingInfo>) {
    let [timer_area, project_area, task_area, description_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(tracker_area);

    frame.render_widget(Block::new().borders(Borders::BOTTOM), tracker_area);

    let status = match tracking_info {
        Some(_) => Span::styled("TRACKING", style::Color::Green),
        None => Span::styled("NOT TRACKING", style::Color::LightRed),
    };
    let duration = Span::styled(
        tracking_info
            .as_ref()
            .map_or("".to_owned(), |i| i.duration.clone()),
        style::Color::Green,
    );

    let columns = [Constraint::Length(14), Constraint::Min(9)];
    let [a1, a2] = Layout::horizontal(columns).areas(timer_area);
    frame.render_widget(Paragraph::new(status), a1);
    frame.render_widget(Paragraph::new(duration), a2);

    let [a1, a2] = Layout::horizontal(columns).areas(project_area);
    frame.render_widget(Paragraph::new("Project:"), a1);
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::raw(
            tracking_info
                .as_ref()
                .map_or("".to_owned(), |i| i.project_name.clone()),
        )])),
        a2,
    );
    // frame.render_widget(
    //     Paragraph::new(Line::from(vec![
    //         Span::raw("Project: "),
    //         Span::raw(
    //             tracking_info
    //                 .as_ref()
    //                 .map_or("".to_owned(), |i| i.project_name.clone()),
    //         ),
    //     ])),
    //     project_area,
    // );

    let [a1, a2] = Layout::horizontal(columns).areas(task_area);
    frame.render_widget(Paragraph::new("Task:"), a1);
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::raw(
            tracking_info
                .as_ref()
                .map_or("".to_owned(), |i| i.task.clone()),
        )])),
        a2,
    );
    // frame.render_widget(
    //     Paragraph::new(Line::from(vec![
    //         Span::raw("Task: "),
    //         Span::raw(
    //             tracking_info
    //                 .as_ref()
    //                 .map_or("".to_owned(), |i| i.task.clone()),
    //         ),
    //     ])),
    //     task_area,
    // );
    let [a1, a2] = Layout::horizontal(columns).areas(description_area);
    frame.render_widget(Paragraph::new("Description:"), a1);
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::raw(
            tracking_info.map_or("".to_owned(), |i| i.description),
        )])),
        a2,
    );
    // frame.render_widget(
    //     Paragraph::new(tracking_info.map_or("".to_owned(), |i| i.description)),
    //     description_area,
    // );
}
