mod adliswil;
mod we_recycle;
use chrono::{Datelike, NaiveDate};
use core::fmt;

use crate::error::GstaldergeistError;
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrashType {
    WeRecycle,
    Normal,
    Bio,
    Cardboard,
    Paper,
}

impl TrashType {
    /// The integer discriminant used to persist a `TrashType` in SQLite. This is
    /// the single source of truth shared by the `ToSql`/`FromSql` impls so the
    /// two encodings cannot drift apart and corrupt stored rows.
    fn as_i64(self) -> i64 {
        match self {
            TrashType::WeRecycle => 0,
            TrashType::Normal => 1,
            TrashType::Bio => 2,
            TrashType::Cardboard => 3,
            TrashType::Paper => 4,
        }
    }

    fn from_i64(value: i64) -> Option<Self> {
        match value {
            0 => Some(TrashType::WeRecycle),
            1 => Some(TrashType::Normal),
            2 => Some(TrashType::Bio),
            3 => Some(TrashType::Cardboard),
            4 => Some(TrashType::Paper),
            _ => None,
        }
    }
}

impl rusqlite::types::FromSql for TrashType {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        TrashType::from_i64(value.as_i64()?).ok_or(rusqlite::types::FromSqlError::InvalidType)
    }
}

impl rusqlite::types::ToSql for TrashType {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::Owned(
            rusqlite::types::Value::Integer(self.as_i64()),
        ))
    }
}

#[derive(Deserialize, Debug)]
struct ChatResult {
    result: ChatInfo,
}

#[derive(Deserialize, Debug)]
struct ChatInfo {
    title: String,
}

#[async_trait]
pub trait WasteGrabber: Send + Sync {
    async fn get_trashes(
        &self,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<HashMap<NaiveDate, Vec<TrashType>>, GstaldergeistError>;
}

impl fmt::Display for TrashType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TrashType::WeRecycle => write!(f, "WeRecycle"),
            TrashType::Normal => write!(f, "Normal"),
            TrashType::Bio => write!(f, "Bio"),
            TrashType::Cardboard => write!(f, "Cardboard"),
            TrashType::Paper => write!(f, "Paper"),
        }
    }
}

#[derive(Debug)]
pub struct TrashesSchedule {
    pub dates: HashMap<NaiveDate, Vec<TrashType>>,
    pub tomorrow_master_name: String,
    pub tomorrow_master_id: i64,
}

/// Selects the flatmate on trash duty for `date`. Duty rotates by one slot per
/// ISO week, wrapping around the list of flatmates.
fn food_master_id(flatmates: &[i64], date: NaiveDate) -> i64 {
    flatmates[(1 + date.iso_week().week0() as usize) % flatmates.len()]
}

fn tomorrow_food_master_id(config: &super::Config) -> i64 {
    let tomorrow = chrono::Local::now().naive_local().date() + chrono::Duration::days(1);
    food_master_id(&config.flatmates, tomorrow)
}

const FOOD_MASTER_TITLE_PREFIX_LEN: usize = 17;

// Skip by chars, not bytes, so a short or multi-byte title never panics.
fn food_master_name_from_title(title: &str) -> String {
    title.chars().skip(FOOD_MASTER_TITLE_PREFIX_LEN).collect()
}

pub async fn grab_tomorrow_food_master_name(config: &super::Config) -> String {
    let client = reqwest::Client::new();

    let bot_token = &config.bot_token;
    let chat_id = tomorrow_food_master_id(config);

    let url = format!(
        "https://api.telegram.org/bot{}/getChat?chat_id={}",
        bot_token, chat_id
    );

    let response = match client.get(url).send().await {
        Ok(response) => response.json::<ChatResult>().await,
        Err(_) => return "Error".to_string(),
    };
    match response {
        Ok(response) => food_master_name_from_title(&response.result.title),
        Err(_) => "Error".to_string(),
    }
}

pub async fn get_trashes(
    config: &super::Config,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<TrashesSchedule, GstaldergeistError> {
    // unfortunately traits do not implement async
    let adliswil_grabber = adliswil::AdliswilWasteGrabber {};
    let we_recycle_grabber = we_recycle::WeRecycleWasteGrabber {};

    let grabbers: Vec<Box<dyn WasteGrabber>> =
        vec![Box::new(adliswil_grabber), Box::new(we_recycle_grabber)];

    let mut dates: HashMap<NaiveDate, Vec<TrashType>> = HashMap::new();
    for grabber in grabbers {
        for (date, trash) in grabber.get_trashes(from, to).await? {
            dates.entry(date).or_default().extend(trash);
        }
    }

    Ok(TrashesSchedule {
        dates,
        tomorrow_master_name: grab_tomorrow_food_master_name(config).await,
        tomorrow_master_id: tomorrow_food_master_id(config),
    })
}

#[cfg(test)]
mod tests {
    use super::{TrashType, food_master_id, food_master_name_from_title};
    use chrono::NaiveDate;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    const ALL_TRASH_TYPES: [TrashType; 5] = [
        TrashType::WeRecycle,
        TrashType::Normal,
        TrashType::Bio,
        TrashType::Cardboard,
        TrashType::Paper,
    ];

    #[test]
    fn trash_type_i64_encoding_round_trips() {
        for trash in ALL_TRASH_TYPES {
            assert_eq!(TrashType::from_i64(trash.as_i64()), Some(trash));
        }
    }

    #[test]
    fn trash_type_discriminants_are_stable() {
        // These map onto already-persisted SQLite rows; changing them silently
        // reinterprets stored history, so pin them down.
        assert_eq!(TrashType::WeRecycle.as_i64(), 0);
        assert_eq!(TrashType::Normal.as_i64(), 1);
        assert_eq!(TrashType::Bio.as_i64(), 2);
        assert_eq!(TrashType::Cardboard.as_i64(), 3);
        assert_eq!(TrashType::Paper.as_i64(), 4);
    }

    #[test]
    fn unknown_trash_type_discriminant_is_rejected() {
        assert_eq!(TrashType::from_i64(5), None);
        assert_eq!(TrashType::from_i64(-1), None);
    }

    #[test]
    fn drops_the_fixed_prefix() {
        assert_eq!(food_master_name_from_title("Gstaldergeist || Alice"), "Alice");
    }

    #[test]
    fn short_title_yields_empty_name_instead_of_panicking() {
        assert_eq!(food_master_name_from_title("too short"), "");
        assert_eq!(food_master_name_from_title(""), "");
    }

    #[test]
    fn prefix_length_boundary_yields_empty_name() {
        assert_eq!(food_master_name_from_title("17_characters_xyz"), "");
    }

    #[test]
    fn counts_characters_not_bytes_for_multibyte_names() {
        assert_eq!(
            food_master_name_from_title("Gstaldergeist || Élodie"),
            "Élodie"
        );
    }

    #[test]
    fn picks_expected_flatmate_for_known_weeks() {
        let flatmates = [10, 20, 30];
        // 2024-01-01 (Mon) is ISO week 1 -> week0 == 0 -> index (1 + 0) % 3 == 1.
        assert_eq!(food_master_id(&flatmates, date(2024, 1, 1)), 20);
        // 2024-01-08 (Mon) is ISO week 2 -> index (1 + 1) % 3 == 2.
        assert_eq!(food_master_id(&flatmates, date(2024, 1, 8)), 30);
        // 2024-01-15 (Mon) is ISO week 3 -> index (1 + 2) % 3 == 0, wrapping around.
        assert_eq!(food_master_id(&flatmates, date(2024, 1, 15)), 10);
    }

    #[test]
    fn duty_advances_by_one_slot_each_week() {
        let flatmates = [10, 20, 30];
        let this_week = food_master_id(&flatmates, date(2024, 1, 1));
        let next_week = food_master_id(&flatmates, date(2024, 1, 8));

        let this_idx = flatmates.iter().position(|&id| id == this_week).unwrap();
        let next_idx = flatmates.iter().position(|&id| id == next_week).unwrap();
        assert_eq!(next_idx, (this_idx + 1) % flatmates.len());
    }

    #[test]
    fn same_iso_week_picks_same_flatmate() {
        let flatmates = [10, 20, 30];
        // Both dates fall in the same ISO week (Mon 2024-01-01 .. Sun 2024-01-07).
        assert_eq!(
            food_master_id(&flatmates, date(2024, 1, 1)),
            food_master_id(&flatmates, date(2024, 1, 7))
        );
    }

    #[test]
    fn single_flatmate_is_always_selected() {
        let flatmates = [42];
        assert_eq!(food_master_id(&flatmates, date(2024, 1, 1)), 42);
        assert_eq!(food_master_id(&flatmates, date(2024, 6, 30)), 42);
    }
}
