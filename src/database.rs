use chrono::Timelike;

type EntityId = u32;

struct TimeEntry {
    id: EntityId,
    project_id: EntityId,
    task_id: EntityId,
    description: String,
    start_time: String,
    end_time: String,
    created_at: String,
}

impl TimeEntry {
    pub fn new(project_id: EntityId, task_id: EntityId, description: String) -> Self {
        let now = chrono::Utc::now();
        let now_str = if now.nanosecond() == 0 {
            now.to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
        } else {
            now.to_rfc3339_opts(chrono::SecondsFormat::Millis, false)
        };

        TimeEntry {
            id: 0, // This should be set by the database when inserting
            project_id,
            task_id,
            description,
            start_time: now_str.clone(),
            end_time: String::new(), // This will be set when the entry is stopped
            created_at: now_str,
        }
    }
}

struct Database {}

impl Database {
    fn new() -> Self {
        Self {}
    }

    fn get_connection() -> rusqlite::Connection {
        let con = rusqlite::Connection::open("~/.local/share/time-tracker/data-rs.db").unwrap();
        con
    }

    fn get_python_utc_now() -> String {
        let now = chrono::Utc::now();
        if now.nanosecond() == 0 {
            now.to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
        } else {
            now.to_rfc3339_opts(chrono::SecondsFormat::Millis, false)
        }
    }

    fn create_time_entry(
        &self,
        project_id: EntityId,
        description: String,
        task_id: Option<EntityId>,
    ) {
        // Logic to insert the time entry into the database
        let con: rusqlite::Connection = Self::get_connection();
        let now = Self::get_python_utc_now();
        let stmt = con.execute(
            "INSERT INTO time_entries 
            (project_id, task_id, description, start_time, end_time, created_at) 
            VALUES (?, ?, ?, ?, NULL, ?)",
            (project_id, task_id, description, now.clone(), now),
        );
    }
}
