use chrono::{Datelike, Timelike};
use resolve_path::PathResolveExt;
use rusqlite::OptionalExtension;

use crate::database;

pub type EntityId = i64;
pub type Timezone = chrono::Utc;
pub type Timestamp = chrono::DateTime<Timezone>;

fn get_now_utc_py() -> String {
    let now = chrono::Utc::now();
    ts_to_str(now)
}

fn ts_to_str(ts: Timestamp) -> String {
    if ts.nanosecond() == 0 {
        ts.to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
    } else {
        ts.to_rfc3339_opts(chrono::SecondsFormat::Millis, false)
    }
}

fn string_to_ts(s: String) -> Timestamp {
    chrono::DateTime::parse_from_rfc3339(s.as_str())
        .unwrap()
        .with_timezone(&chrono::Utc)
}

#[derive(Clone)]
pub struct Project {
    pub id: EntityId,
    pub name: String,
    pub color: String, // hex color
    pub description: String,
    pub created_at: Timestamp,
}

#[derive(Default, Clone)]
pub struct TimeEntry {
    pub id: EntityId,
    pub project_id: EntityId,
    pub task_id: Option<EntityId>,
    pub description: String,
    pub start_time: Timestamp,
    pub end_time: Option<Timestamp>,
    pub created_at: Timestamp,
}

impl TimeEntry {
    pub fn duration(&self) -> chrono::Duration {
        self.end_time.unwrap_or(Timezone::now()) - self.start_time
    }
}

#[derive(Default)]
pub struct DailyStats {
    pub entries_count: usize,
    pub total_time: chrono::TimeDelta,
    pub longest_session: chrono::TimeDelta,
}

#[derive(Default)]
pub struct IntervalMetrics {
    pub total_time: chrono::TimeDelta,
    pub session_count: usize,
}

#[derive(Default)]
pub struct Metrics {
    pub today: DailyStats,
    pub today_metrics: IntervalMetrics,
    pub week_metrics: IntervalMetrics,
    pub month_metrics: IntervalMetrics,
    pub total_metrics: IntervalMetrics,
}

impl TimeEntry {
    pub fn new(project_id: EntityId, description: String, task_id: Option<EntityId>) -> Self {
        let now = chrono::Utc::now();
        TimeEntry {
            id: 0, // This should be set by the database when inserting
            project_id,
            task_id,
            description,
            start_time: now,
            end_time: None,
            created_at: now,
        }
    }
}

pub struct Database {
    con: rusqlite::Connection,
}

impl Database {
    pub fn new(db_path: String) -> rusqlite::Result<Self> {
        let con = Self::connect(db_path)?;
        Ok(Self { con })
    }

    fn connect(db_path: String) -> rusqlite::Result<rusqlite::Connection> {
        let db_file = db_path.resolve();
        rusqlite::Connection::open(db_file)
    }

    fn get_connection(&self) -> &rusqlite::Connection {
        &self.con
    }

    pub fn start_entry(
        &self,
        project_id: EntityId,
        description: String,
        task_id: Option<EntityId>,
    ) -> rusqlite::Result<TimeEntry> {
        // Logic to insert the time entry into the database
        let con = self.get_connection();
        let mut entry = TimeEntry::new(project_id, description, task_id);
        con.execute(
            "INSERT INTO time_entries
            (project_id, task_id, description, start_time, end_time, created_at)
            VALUES (?, ?, ?, ?, NULL, ?)",
            (
                &entry.project_id,
                &entry.task_id,
                &entry.description,
                ts_to_str(entry.start_time),
                ts_to_str(entry.created_at),
            ),
        )?;

        entry.id = con.last_insert_rowid();
        Ok(entry)
    }

    pub fn stop_entry(&self, entry_id: EntityId) -> rusqlite::Result<()> {
        let con = self.get_connection();
        let now = get_now_utc_py();
        con.execute(
            "UPDATE time_entries SET end_time = ? WHERE id = ? AND end_time IS NULL",
            (now, entry_id),
        )?;
        Ok(())
    }

    pub fn get_running_entry(&self) -> rusqlite::Result<Option<TimeEntry>> {
        let con = self.get_connection();
        con.query_row(
            "SELECT id, project_id, task_id, description, start_time, end_time, created_at
             FROM time_entries WHERE end_time IS NULL ORDER BY start_time DESC LIMIT 1",
            [],
            |row| {
                Ok(TimeEntry {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    task_id: row.get(2).unwrap_or(None),
                    description: row.get(3)?,
                    start_time: string_to_ts(row.get(4)?),
                    end_time: row.get(5).map_or(None, |ts| Some(string_to_ts(ts))),
                    created_at: string_to_ts(row.get(6)?),
                })
            },
        )
        .optional()
    }

    pub fn update_entry(&self, updated_entry: TimeEntry) -> rusqlite::Result<()> {
        let con = self.get_connection();
        con.execute(
            "UPDATE time_entries SET project_id=?, task_id=?, description=?, start_time=?, end_time=? 
             WHERE id=?",
            (
                &updated_entry.project_id,
                &updated_entry.task_id,
                &updated_entry.description,
                ts_to_str(updated_entry.start_time),
                updated_entry.end_time.map(ts_to_str),
                &updated_entry.id
            ),
        )?;
        Ok(())
    }

    pub fn get_recent_entries(
        &self,
        limit: Option<u32>,
        include_running_entry: bool,
    ) -> rusqlite::Result<Vec<TimeEntry>> {
        let con = self.get_connection();

        let query = format!(
            "SELECT id, project_id, task_id, description, start_time, end_time, created_at
                     FROM time_entries {}
                     ORDER BY start_time DESC {}",
            if include_running_entry {
                ""
            } else {
                "WHERE end_time IS NOT NULL"
            },
            match limit {
                Some(_) => "LIMIT ?",
                None => "",
            },
        ); // include the active one in the list
        match limit {
            Some(limit) => {
                let mut stmt = con.prepare(&query)?;

                let entries = stmt.query_map([limit], |row| {
                    Ok(TimeEntry {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        task_id: row.get(2).unwrap_or(None),
                        description: row.get(3)?,
                        start_time: string_to_ts(row.get(4)?),
                        end_time: row.get(5).map_or(None, |ts| Some(string_to_ts(ts))),
                        created_at: string_to_ts(row.get(6)?),
                    })
                })?;
                Ok(entries.filter_map(|e| e.ok()).collect())
            }
            None => {
                let mut stmt = con.prepare(&query)?;

                let entries = stmt.query_map([], |row| {
                    Ok(TimeEntry {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        task_id: row.get(2).unwrap_or(None),
                        description: row.get(3)?,
                        start_time: string_to_ts(row.get(4)?),
                        end_time: row.get(5).map_or(None, |ts| Some(string_to_ts(ts))),
                        created_at: string_to_ts(row.get(6)?),
                    })
                })?;
                Ok(entries.filter_map(|e| e.ok()).collect())
            }
        }
    }

    pub fn get_projects(&self) -> rusqlite::Result<Vec<Project>> {
        let con = self.get_connection();
        let mut stmt = con.prepare(
            "SELECT id, name, color, description, created_at FROM projects ORDER BY name",
        )?;

        let projects = stmt.query_map([], |row| {
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                description: row.get(3)?,
                created_at: string_to_ts(row.get(4)?),
            })
        })?;
        Ok(projects.filter_map(|p| p.ok()).collect())
    }

    // TODO Edge case, but unrealistic: handle tracking over midnight
    pub fn get_metrics_summary(&self) -> rusqlite::Result<Metrics> {
        let all_entries = self.get_recent_entries(None, false)?;

        // Helper days
        let today = Timezone::now();
        let this_week_first_day = today
            .checked_sub_days(chrono::Days::new(
                today.weekday().number_from_monday() as u64
            ))
            .expect("Unable to calculate fist week day");
        let this_week_last_day = today
            .checked_add_days(chrono::Days::new(
                today.weekday().number_from_monday() as u64
            ))
            .expect("Unable to calculate last week day");

        // Filter entries by time interval

        let this_month_entries = all_entries.iter().filter(|e| {
            e.start_time.year() == today.year() && e.start_time.month() == today.month()
        });
        let this_week_entries = all_entries
            .iter()
            .filter(|e| this_week_first_day <= e.start_time && e.start_time <= this_week_last_day);
        let today_entries = this_week_entries
            .clone()
            .filter(|e| today.day() == e.start_time.day());

        // Condense metrics per interval

        let today_longest: chrono::TimeDelta = today_entries
            .clone()
            .map(|e| e.end_time.unwrap_or(Timezone::now()) - e.start_time)
            .max()
            .unwrap_or(chrono::TimeDelta::zero());
        let today_time_tracked: chrono::TimeDelta = today_entries
            .clone()
            .map(|e| e.end_time.unwrap_or(Timezone::now()) - e.start_time)
            .sum();

        let today_entries_count = today_entries.clone().count();
        let today_stats = DailyStats {
            entries_count: today_entries_count,
            total_time: today_time_tracked,
            longest_session: today_longest,
        };

        let today_metrics = IntervalMetrics {
            total_time: today_entries
                .map(|e| e.end_time.unwrap_or(Timezone::now()) - e.start_time)
                .sum(),
            session_count: today_entries_count,
        };

        let week_metrics = IntervalMetrics {
            total_time: this_week_entries
                .clone()
                .map(|e| e.end_time.unwrap_or(Timezone::now()) - e.start_time)
                .sum(),
            session_count: this_week_entries.count(),
        };
        let month_metrics = IntervalMetrics {
            total_time: this_month_entries
                .clone()
                .map(|e| e.end_time.unwrap_or(Timezone::now()) - e.start_time)
                .sum(),
            session_count: this_month_entries.count(),
        };

        let total_metrics = IntervalMetrics {
            total_time: all_entries
                .clone()
                .iter()
                .map(|e| e.end_time.unwrap_or(Timezone::now()) - e.start_time)
                .sum(),
            session_count: all_entries.len(),
        };

        Ok(Metrics {
            today: today_stats,
            today_metrics,
            week_metrics,
            month_metrics,
            total_metrics,
        })
    }
}
