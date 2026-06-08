mod adliswil;
mod we_recycle;
use chrono::{Datelike, NaiveDate};
use core::fmt;

use crate::error::GstaldergeistError;
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug)]
pub enum TrashType {
    WeRecycle,
    Normal,
    Bio,
    Cardboard,
    Paper,
}

// impl TrashType from rusqlite
impl rusqlite::types::FromSql for TrashType {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        match value.as_i64()? {
            0 => Ok(TrashType::WeRecycle),
            1 => Ok(TrashType::Normal),
            2 => Ok(TrashType::Bio),
            3 => Ok(TrashType::Cardboard),
            4 => Ok(TrashType::Paper),
            _ => Err(rusqlite::types::FromSqlError::InvalidType),
        }
    }
}

impl rusqlite::types::ToSql for TrashType {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::Owned(
            rusqlite::types::Value::Integer(match self {
                TrashType::WeRecycle => 0,
                TrashType::Normal => 1,
                TrashType::Bio => 2,
                TrashType::Cardboard => 3,
                TrashType::Paper => 4,
            }),
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

pub async fn grab_tomorrow_food_master_name(config: &super::Config) -> String {
    let client = reqwest::Client::new();

    let bot_token = &config.bot_token;
    let chat_id = tomorrow_food_master_id(config);

    let url = format!(
        "https://api.telegram.org/bot{}/getChat?chat_id={}",
        bot_token, chat_id
    );

    let response = client
        .get(url)
        .send()
        .await
        .unwrap()
        .json::<ChatResult>()
        .await;
    match response {
        Ok(response) => {
            let mut chat_info = response.result;
            chat_info.title.split_off(17)
        }
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
    use super::food_master_id;
    use chrono::NaiveDate;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
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
