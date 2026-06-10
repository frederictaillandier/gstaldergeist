use crate::data_grabber::TrashType;
use crate::error::GstaldergeistError;
use chrono::NaiveDate;
use rusqlite::Connection;
use std::collections::HashMap;

const DB_PATH: &str = "/data/gstaldergeist.db";

/// Open the database and make sure the `trashes` table exists.
fn open_db() -> Result<Connection, GstaldergeistError> {
    let conn = Connection::open(DB_PATH)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS trashes (date DATE, waste_type INTEGER)",
        [],
    )?;
    Ok(conn)
}

pub fn get_all_trashes() -> Result<HashMap<NaiveDate, Vec<TrashType>>, GstaldergeistError> {
    let conn = open_db()?;
    let mut stmt = conn.prepare("SELECT date, waste_type FROM trashes")?;
    let rows = stmt.query_map([], |row| {
        let date: NaiveDate = row.get(0)?;
        let waste_type: TrashType = row.get(1)?;
        Ok((date, waste_type))
    })?;
    let mut trashes: HashMap<NaiveDate, Vec<TrashType>> = HashMap::new();
    for row in rows {
        let (date, waste_type) = row?;
        trashes.entry(date).or_default().push(waste_type);
    }
    Ok(trashes)
}

pub fn set_trashes(trashes: &HashMap<NaiveDate, Vec<TrashType>>) -> Result<(), GstaldergeistError> {
    let conn = open_db()?;

    // delete all rows
    conn.execute("DELETE FROM trashes", [])?;

    let mut stmt = conn.prepare("INSERT INTO trashes (date, waste_type) VALUES (?1, ?2)")?;
    for (date, waste_types) in trashes {
        for waste_type in waste_types {
            stmt.execute(rusqlite::params![date, waste_type])?;
        }
    }
    Ok(())
}
