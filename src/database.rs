use crate::data_grabber::TrashType;
use crate::error::GstaldergeistError;
use chrono::NaiveDate;
use rusqlite::Connection;
use std::collections::HashMap;

const DB_PATH: &str = "/data/gstaldergeist.db";

/// Open the database and make sure the `trashes` table exists.
fn open_db() -> Result<Connection, GstaldergeistError> {
    let conn = Connection::open(DB_PATH)?;
    init_schema(&conn)?;
    Ok(conn)
}

fn init_schema(conn: &Connection) -> Result<(), GstaldergeistError> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS trashes (date DATE, waste_type INTEGER)",
        [],
    )?;
    Ok(())
}

fn read_all_trashes(
    conn: &Connection,
) -> Result<HashMap<NaiveDate, Vec<TrashType>>, GstaldergeistError> {
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

fn write_trashes(
    conn: &Connection,
    trashes: &HashMap<NaiveDate, Vec<TrashType>>,
) -> Result<(), GstaldergeistError> {
    conn.execute("DELETE FROM trashes", [])?;

    let mut stmt = conn.prepare("INSERT INTO trashes (date, waste_type) VALUES (?1, ?2)")?;
    for (date, waste_types) in trashes {
        for waste_type in waste_types {
            stmt.execute(rusqlite::params![date, waste_type])?;
        }
    }
    Ok(())
}

pub fn get_all_trashes() -> Result<HashMap<NaiveDate, Vec<TrashType>>, GstaldergeistError> {
    read_all_trashes(&open_db()?)
}

pub fn set_trashes(trashes: &HashMap<NaiveDate, Vec<TrashType>>) -> Result<(), GstaldergeistError> {
    write_trashes(&open_db()?, trashes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        conn
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn display_sorted(types: &[TrashType]) -> Vec<String> {
        let mut names: Vec<String> = types.iter().map(|t| t.to_string()).collect();
        names.sort();
        names
    }

    #[test]
    fn empty_table_reads_back_empty() {
        let conn = mem_db();
        assert!(read_all_trashes(&conn).unwrap().is_empty());
    }

    #[test]
    fn written_trashes_round_trip() {
        let conn = mem_db();
        let mut input = HashMap::new();
        input.insert(date(2026, 6, 12), vec![TrashType::Normal, TrashType::Bio]);
        input.insert(date(2026, 6, 14), vec![TrashType::Paper]);

        write_trashes(&conn, &input).unwrap();
        let output = read_all_trashes(&conn).unwrap();

        assert_eq!(output.len(), 2);
        for (date, types) in &input {
            assert_eq!(display_sorted(&output[date]), display_sorted(types));
        }
    }

    #[test]
    fn write_replaces_previous_contents() {
        let conn = mem_db();

        let mut first = HashMap::new();
        first.insert(date(2026, 6, 12), vec![TrashType::Cardboard]);
        write_trashes(&conn, &first).unwrap();

        let mut second = HashMap::new();
        second.insert(date(2026, 6, 13), vec![TrashType::WeRecycle]);
        write_trashes(&conn, &second).unwrap();

        let output = read_all_trashes(&conn).unwrap();
        assert_eq!(output.len(), 1);
        assert!(!output.contains_key(&date(2026, 6, 12)));
        assert_eq!(display_sorted(&output[&date(2026, 6, 13)]), vec!["WeRecycle"]);
    }
}
