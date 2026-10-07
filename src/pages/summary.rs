use chrono::Datelike;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use itertools::Itertools;
use ratatui::layout::Layout;
use ratatui::style::{self, Stylize};
use ratatui::symbols::bar;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    self, Bar, Block, BorderType, Borders, Cell, Padding, Paragraph, Row, Table,
};
use ratatui::{Frame, layout::Rect};
use ratatui::{layout::Constraint, style::Style};
use std::collections::HashMap;
use std::str::FromStr;
use tui_barchart_ext::bar_symbols::{HALF, HORIZONTAL_HALF};

use crate::database::{self, Timezone};
use crate::pages::{format_duration_hm_verbose, format_duration_hms_verbose};
use crate::theme;

pub struct Summary {
    entries: Vec<database::TimeEntry>,
    projects: Vec<database::Project>,
    metrics: database::Metrics,
    year: i32,
    month: u32,
    counter: i32,
}

impl Summary {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            projects: Vec::new(),
            metrics: database::Metrics::default(),
            year: Timezone::now().year(),
            month: Timezone::now().month(),
            counter: 0,
        }
    }

    pub fn refresh(
        &mut self,
        entries: Vec<database::TimeEntry>,
        projects: Vec<database::Project>,
        metrics: database::Metrics,
    ) {
        self.entries = entries;
        self.projects = projects;
        self.metrics = metrics
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let [header_area, overview_area, stats_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(15),
            Constraint::Min(8),
        ])
        .areas(area);
        let [chart_area, _] =
            Layout::horizontal([Constraint::Length(6 + (4 * 31)), Constraint::Min(0)])
                .areas(overview_area);

        frame.render_widget(
            Paragraph::new(vec![
                Line::from(format!(
                    "< {} >",
                    chrono::NaiveDate::from_ymd_opt(self.year, self.month, 1)
                        .unwrap()
                        .format("%B %Y")
                ))
                .centered(),
            ]),
            header_area,
        );

        self.render_monthly_barchart(frame, overview_area, self.month, self.year);

        let details_block = Block::bordered()
            .border_type(BorderType::Rounded)
            .padding(Padding::horizontal(1))
            .title(" Details ");

        let monthly_time_per_project = self
            .entries
            .iter()
            .filter(|e| e.start_time.year() == self.year && e.start_time.month() == self.month)
            .into_group_map_by(|e| e.project_id)
            .into_iter()
            .map(|(project_id, entries)| {
                let project = self.projects.iter().find(|p| p.id == project_id).unwrap();
                let total_time = entries.iter().map(|e| e.duration()).sum();
                (project.clone(), total_time)
            })
            .sorted_by(|(p1, _), (p2, _)| Ord::cmp(&p1.id, &p2.id))
            .collect::<Vec<(database::Project, chrono::TimeDelta)>>();

        let column_widths = [Constraint::Length(25), Constraint::Length(13)];
        let details_table = Table::new(
            monthly_time_per_project.iter().map(|(p, h)| {
                Row::new([
                    Cell::from(p.name.clone()).style(p.color),
                    Cell::from(
                        Span::from(format_duration_hms_verbose(h)).into_right_aligned_line(),
                    ),
                ])
            }),
            column_widths,
        );
        let bars = monthly_time_per_project
            .iter()
            .map(|(p, h)| {
                tui_barchart_ext::barchart::Bar::with_label(
                    Line::from(p.name.clone()).fg(p.color),
                    h.num_hours() as u64,
                )
                .text_value("")
                .style(p.color)
            })
            .collect::<Vec<tui_barchart_ext::barchart::Bar>>();

        let details_area = details_block.inner(stats_area);
        const HORIZONTAL_HALF_BAR: tui_barchart_ext::bar_symbols::Set =
            tui_barchart_ext::bar_symbols::Set {
                full: HALF,
                seven_eighths: HALF,
                three_quarters: HALF,
                five_eighths: HALF,
                half: HALF,
                three_eighths: HALF,
                one_quarter: HALF,
                one_eighth: HALF,
                empty: " ",
            };
        let monthly_total: chrono::Duration = monthly_time_per_project.iter().map(|(_, t)| t).sum();
        let [chart_area, numbers_area] =
            Layout::horizontal([Constraint::Min(0), Constraint::Length(16)])
                .areas(details_block.inner(details_area));
        let project_times_chart = tui_barchart_ext::barchart::BarChart::new(bars)
            .direction(ratatui::layout::Direction::Horizontal)
            .bar_width(1)
            .bar_set(HORIZONTAL_HALF_BAR)
            .bar_gap(0)
            .bar_style(Style::new().light_blue())
            .max(monthly_total.num_hours() as u64)
            .value_style(Style::new().italic().bold())
            .label_style(Style::new().white());
        frame.render_widget(project_times_chart, chart_area);
        frame.render_widget(
            Paragraph::new(
                monthly_time_per_project
                    .iter()
                    .map(|(p, h)| {
                        Line::from(vec![
                            Span::from(format_duration_hm_verbose(h)),
                            Span::from(format!(
                                "  {:.1}%",
                                if monthly_total == chrono::Duration::zero() {
                                    0.0
                                } else {
                                    100.0 * h.num_seconds() as f64
                                        / monthly_total.num_seconds() as f64
                                }
                            ))
                            .fg(theme::GREEN),
                        ])
                        .right_aligned()
                    })
                    .collect::<Vec<Line>>(),
            ),
            // .block(details_block),
            numbers_area,
        );
        // frame.render_widget(details_table, details_area);
    }

    fn render_monthly_barchart(
        &self,
        frame: &mut Frame<'_>,
        chart_area: Rect,
        month: u32,
        year: i32,
    ) {
        let bar_chart_block = Block::new()
            // .borders(Borders::LEFT | Borders::BOTTOM)
            // .border_type(BorderType::Rounded)
            // .border_style(theme::OVERLAY_1)
            // .borders(Borders::ALL)
            .padding(Padding::new(1, 1, 1, 0))
            .title(" Daily Activity ")
            .title_style(theme::TEXT);

        let first_day = chrono::NaiveDate::from_ymd_opt(self.year, self.month, 1)
            .expect(format!("Failed to get date from {}-{}", month, year).as_str());
        let next_month = first_day
            .checked_add_months(chrono::Months::new(1))
            .unwrap();
        let days_in_month = next_month.signed_duration_since(first_day).num_days() as u32 + 1;

        let this_month_entries = self
            .entries
            .iter()
            .filter(|e| e.start_time.year() == year && e.start_time.month() == month);

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
                                    .map(|e| e.duration())
                                    .sum::<chrono::TimeDelta>(),
                            )
                        })
                        .max_by(|(_, duration1), (_, duration2)| Ord::cmp(duration1, duration2))
                        .map(|(project_id, _)| {
                            self.projects
                                .iter()
                                .find(|p| p.id == project_id)
                                .map_or(style::Color::White, |p| p.color)
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
                let date = chrono::NaiveDate::from_ymd_opt(self.year, self.month, d).expect(
                    format!("Failed to create date from {}.{}.{}", d, month, year).as_str(),
                );

                let weekday = date.weekday();
                let weekday_color = if weekday.num_days_from_monday() == 0 {
                    theme::BLUE
                } else if weekday.num_days_from_monday() >= 5 {
                    theme::RED
                } else {
                    theme::TEXT
                };

                let is_weekday = weekday.num_days_from_monday() < 5;

                if time == 0 && !is_weekday {
                    return tui_barchart_ext::barchart::Bar::with_label(
                        Line::from("..").fg(weekday_color),
                        0,
                    )
                    .text_value("");
                    // return Some(widgets::Bar::with_label("..".to_string(), 0).text_value(""));
                }
                let dominat_projects_color = dominant_project_per_day
                    .get(&d)
                    .map_or(style::Color::White, |c| *c);
                tui_barchart_ext::barchart::Bar::with_label(
                    Line::from(d.to_string()).fg(weekday_color),
                    time,
                )
                .text_value("")
                .style(dominat_projects_color)
            })
            .dedup()
            // .intersperse(widgets::Bar::with_label("..".to_string(), 0).text_value(""))
            .collect::<Vec<tui_barchart_ext::barchart::Bar<'_>>>();
        let charts_inner_area = bar_chart_block.inner(chart_area);

        let summary_chart = tui_barchart_ext::barchart::BarChart::vertical(bars)
            .bar_width(1)
            .block(bar_chart_block)
            .bar_width(2)
            // .bar_gap(
            //     if charts_inner_area.width > ((days_in_month * 3) - 3) as u16 {
            //
            //     } else {
            //         0
            //     },
            // )
            .group_gap(2)
            .bg(theme::SURFACE_0)
            .bar_style(Style::new().light_blue())
            .max(12 * 60)
            .value_style(Style::new().italic().bold())
            .label_style(Style::new().white());
        frame.render_widget(summary_chart, chart_area);
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> bool {
        match key_event {
            KeyEvent {
                code: KeyCode::Char('H') | KeyCode::Left,
                modifiers: KeyModifiers::SHIFT,
                ..
            } => {
                self.year -= 1;
                true
            }
            KeyEvent {
                code: KeyCode::Char('L') | KeyCode::Right,
                modifiers: KeyModifiers::SHIFT,
                ..
            } => {
                self.year += 1;
                true
            }
            _ => match key_event.code {
                KeyCode::Left | KeyCode::Char('h') => {
                    self.counter += 1;
                    if self.month == 1 {
                        self.month = 12;
                        self.year -= 1;
                    }
                    self.month -= 1;
                    true
                }

                KeyCode::Right | KeyCode::Char('l') => {
                    self.counter -= 1;
                    if self.month == 12 {
                        self.month = 1;
                        self.year += 1;
                    }
                    self.month += 1;
                    true
                }
                _ => false,
            },
        }
    }
}
