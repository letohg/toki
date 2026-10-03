use chrono::Datelike;
use crossterm::event::KeyEvent;
use itertools::Itertools;
use ratatui::layout::Layout;
use ratatui::style::{self, Stylize};
use ratatui::symbols::bar;
use ratatui::text::Span;
use ratatui::widgets::{
    self, Bar, Block, BorderType, Borders, Cell, Padding, Paragraph, Row, Table,
};
use ratatui::{Frame, layout::Rect};
use ratatui::{layout::Constraint, style::Style};
use std::collections::HashMap;
use std::str::FromStr;

use crate::database::{self, Timezone};
use crate::pages::format_delta_time_statistic;

pub struct Summary {
    entries: Vec<database::TimeEntry>,
    projects: Vec<database::Project>,
}

impl Summary {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            projects: Vec::new(),
        }
    }

    pub fn refresh(&mut self, entries: Vec<database::TimeEntry>, projects: Vec<database::Project>) {
        self.entries = entries;
        self.projects = projects;
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let block = Block::bordered()
            .border_type(widgets::BorderType::Rounded)
            .title(" Summary ");

        let [overview_area, stats_area] =
            Layout::vertical([Constraint::Length(15), Constraint::Min(8)]).areas(block.inner(area));
        let [chart_area, _] =
            Layout::horizontal([Constraint::Length(6 + (4 * 31)), Constraint::Min(0)])
                .areas(overview_area);

        frame.render_widget(block, area);
        self.render_monthly_barchart(frame, chart_area);

        let details_block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(" Details ");

        let now = Timezone::now();
        let this_month = now.month();
        let montly_time_per_project = self
            .entries
            .iter()
            .filter(|e| e.start_time.year() == now.year() && e.start_time.month() == this_month)
            .into_group_map_by(|e| e.project_id)
            .into_iter()
            .map(|(project_id, entries)| {
                let project = self.projects.iter().find(|p| p.id == project_id).unwrap();
                let total_time = entries
                    .iter()
                    .map(|e| e.end_time.unwrap_or(now) - e.start_time)
                    .sum();
                (project.clone(), total_time)
            })
            .sorted_by(|(p1, _), (p2, _)| Ord::cmp(&p1.id, &p2.id))
            .collect::<Vec<(database::Project, chrono::TimeDelta)>>();

        let column_widths = [Constraint::Length(25), Constraint::Length(13)];
        let details_table = Table::new(
            montly_time_per_project.iter().map(|(p, h)| {
                Row::new([
                    Cell::from(p.name.clone())
                        .style(style::Color::from_str(&p.color).unwrap_or_default()),
                    Cell::from(
                        Span::from(format_delta_time_statistic(h)).into_right_aligned_line(),
                    ),
                ])
            }),
            column_widths,
        );

        frame.render_widget(&details_block, stats_area);
        frame.render_widget(details_table, details_block.inner(stats_area));
    }

    fn render_monthly_barchart(&self, frame: &mut Frame<'_>, chart_area: Rect) {
        let bar_chart_block = Block::bordered()
            .border_type(widgets::BorderType::Rounded)
            .padding(Padding::symmetric(2, 1))
            .title(" This Month ");

        let now = Timezone::now();
        let first_day: chrono::DateTime<Timezone> = now.with_day(1).unwrap();
        let this_month = now.month();
        let next_month = first_day
            .checked_add_months(chrono::Months::new(1))
            .unwrap();
        let days_in_month = next_month.signed_duration_since(first_day).num_days() as u32 + 1;

        let this_month_entries = self
            .entries
            .iter()
            .filter(|e| e.start_time.year() == now.year() && e.start_time.month() == this_month);

        let time_per_day = this_month_entries
            .clone()
            .into_group_map_by(|e| e.start_time.day())
            .into_iter()
            .map(|(k, v)| {
                (
                    k,
                    v.iter()
                        .map(|e| e.end_time.unwrap_or(Timezone::now()) - e.start_time)
                        .sum(),
                )
            })
            .collect::<HashMap<u32, chrono::TimeDelta>>();

        let dominant_project_per_day = this_month_entries
            .into_group_map_by(|e| e.start_time.day())
            .into_iter()
            .map(|(day, entries_per_day)| {
                (
                    day,
                    entries_per_day
                        .iter()
                        .into_group_map_by(|e| e.project_id)
                        .into_iter()
                        .map(|(project_id, entries)| {
                            (
                                project_id,
                                entries
                                    .iter()
                                    .map(|e| e.end_time.unwrap_or(Timezone::now()) - e.start_time)
                                    .sum::<chrono::TimeDelta>(),
                            )
                        })
                        .max_by(|(_, duration1), (_, duration2)| Ord::cmp(duration1, duration2))
                        .map(|(project_id, _)| {
                            self.projects.iter().find(|p| p.id == project_id).map_or(
                                style::Color::White,
                                |p| {
                                    style::Color::from_str(&p.color)
                                        .unwrap_or(style::Color::Magenta)
                                },
                            )
                        })
                        .unwrap(),
                )
            })
            .collect::<HashMap<u32, style::Color>>();

        let bars = (1..days_in_month)
            .map(|d| {
                let time = time_per_day
                    .get(&d)
                    .map_or(0, |dt| (dt.num_minutes()) as u64);
                let dominat_projects_color = dominant_project_per_day
                    .get(&d)
                    .map_or(style::Color::White, |c| *c);
                widgets::Bar::with_label(d.to_string(), time)
                    .text_value("")
                    .style(dominat_projects_color)
            })
            .collect::<Vec<widgets::Bar<'_>>>();

        let summary_chart = ratatui::widgets::BarChart::new(bars)
            .block(bar_chart_block)
            .bar_width(3)
            .bar_gap(1)
            .group_gap(2)
            .bar_style(Style::new().light_blue())
            .max(12 * 60)
            .value_style(Style::new().italic().bold())
            .label_style(Style::new().white());
        frame.render_widget(summary_chart, chart_area);
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> bool {
        false
    }
}
