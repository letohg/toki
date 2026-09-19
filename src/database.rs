use chrono::Timelike;
use resolve_path::PathResolveExt;
use rusqlite::OptionalExtension;

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

pub struct Project {
    pub id: EntityId,
    pub name: String,
    pub color: String, // hex color
    pub description: String,
    pub created_at: Timestamp,
}

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

pub struct Database {}

impl Database {
    pub fn new() -> Self {
        Self {}
    }
}

pub fn get_connection() -> rusqlite::Connection {
    // let db_file = "~/.local/share/time-tracker/data-rs.db".resolve();
    let db_file = "~/.time_tracker/workspaces/default.db".resolve();
    let con = rusqlite::Connection::open(db_file).unwrap();
    con
}

pub fn start_entry(
    project_id: EntityId,
    description: String,
    task_id: Option<EntityId>,
) -> rusqlite::Result<TimeEntry> {
    // Logic to insert the time entry into the database
    let con: rusqlite::Connection = get_connection();
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

pub fn stop_entry(entry_id: EntityId) -> rusqlite::Result<()> {
    let con = get_connection();
    let now = get_now_utc_py();
    con.execute(
        "UPDATE time_entries SET end_time = ? WHERE id = ? AND end_time IS NULL",
        (now, entry_id),
    )?;
    Ok(())
}

pub fn get_running_entry() -> Option<TimeEntry> {
    let con = get_connection();
    con.query_row(
        "SELECT id, project_id, task_id, description, start_time, end_time, created_at
             FROM time_entries WHERE end_time IS NULL ORDER BY start_time DESC LIMIT 1",
        [],
        |row| {
            Ok(TimeEntry {
                id: row.get(0)?,
                project_id: row.get(1)?,
                task_id: row.get(2)?,
                description: row.get(3)?,
                start_time: string_to_ts(row.get(4)?),
                end_time: row.get(5).map_or(None, |ts| Some(string_to_ts(ts))),
                created_at: string_to_ts(row.get(6)?),
            })
        },
    )
    .optional()
    .unwrap_or(None)
}

pub fn get_recent_entries(limit: u32) -> Vec<TimeEntry> {
    let con = get_connection();

    let mut stmt = con
        .prepare(
            "SELECT id, project_id, task_id, description, start_time, end_time, created_at
         FROM time_entries 
         ORDER BY start_time DESC LIMIT ?",
            // WHERE end_time IS NOT NULL // include the active one in the list
        )
        .unwrap();

    let entries = stmt
        .query_map([limit], |row| {
            Ok(TimeEntry {
                id: row.get(0)?,
                project_id: row.get(1)?,
                task_id: row.get(2)?,
                description: row.get(3)?,
                start_time: string_to_ts(row.get(4)?),
                end_time: row.get(5).map_or(None, |ts| Some(string_to_ts(ts))),
                created_at: string_to_ts(row.get(6)?),
            })
        })
        .unwrap();
    entries.filter_map(|e| e.ok()).collect()
}

pub fn get_projects() -> rusqlite::Result<Vec<Project>> {
    let con = get_connection();
    let mut stmt =
        con.prepare("SELECT id, name, color, description, created_at FROM projects ORDER BY name")?;

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

///////////////////////////////////////////////////////
/// Debugging functions
///////////////////////////////////////////////////////

pub fn test_db_queries() {
    let db_file = "~/.local/share/time-tracker/data-rs.db".resolve();
    println!("Opening: {}", db_file.display());

    // Check the file actually has content
    use std::fs;
    let meta = fs::metadata(&db_file).unwrap();
    println!("Size: {} bytes", meta.len());

    let con = get_connection();
    let count: i64 = con
        .query_row("SELECT COUNT(*) FROM time_entries", [], |r| r.get(0))
        .unwrap();
    println!("Total rows: {}", count);

    let count: i64 = con
        .query_row(
            // "SELECT COUNT(*) FROM time_entries WHERE end_time IS NULL",
            "SELECT COUNT(*)
             FROM time_entries WHERE end_time IS NULL ORDER BY start_time DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    println!("Rows with NULL end_time: {}", count);
}
