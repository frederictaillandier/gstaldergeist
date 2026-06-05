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

async fn tomorrow_food_master_id(config: &super::Config) -> i64 {
    let tomorrow = chrono::Local::now().naive_local().date() + chrono::Duration::days(1);

    let chat_id =
        config.flatmates[(1 + tomorrow.iso_week().week0() as usize) % config.flatmates.len()];
    chat_id
}

/// Number of leading characters in a flatmate's Telegram chat title that make
/// up a fixed label; the food master's display name is whatever follows it.
const FOOD_MASTER_TITLE_PREFIX_LEN: usize = 17;

/// Extracts the food master's display name from their Telegram chat title by
/// dropping the fixed prefix. Unlike `String::split_off`, this never panics on
/// short titles or multi-byte characters: a title shorter than the prefix
/// simply yields an empty name.
fn food_master_name_from_title(title: &str) -> String {
    title.chars().skip(FOOD_MASTER_TITLE_PREFIX_LEN).collect()
}

pub async fn grab_tomorrow_food_master_name(config: &super::Config) -> String {
    let tomorrow = chrono::Local::now().naive_local().date() + chrono::Duration::days(1);
    let client = reqwest::Client::new();

    let bot_token = &config.bot_token;
    let chat_id =
        &config.flatmates[(1 + tomorrow.iso_week().week0() as usize) % config.flatmates.len()];

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
        tomorrow_master_id: tomorrow_food_master_id(config).await,
    })
}

#[cfg(test)]
mod tests {
    use super::food_master_name_from_title;

    #[test]
    fn drops_the_fixed_prefix() {
        // 17-character prefix followed by the actual name.
        assert_eq!(food_master_name_from_title("Gstaldergeist || Alice"), "Alice");
    }

    #[test]
    fn short_title_yields_empty_name_instead_of_panicking() {
        assert_eq!(food_master_name_from_title("too short"), "");
        assert_eq!(food_master_name_from_title(""), "");
    }

    #[test]
    fn prefix_length_boundary_yields_empty_name() {
        // Exactly the prefix length: nothing remains after dropping it.
        assert_eq!(food_master_name_from_title("17_characters_xyz"), "");
    }

    #[test]
    fn counts_characters_not_bytes_for_multibyte_names() {
        // A multi-byte character right after the prefix must not be split mid-byte.
        assert_eq!(
            food_master_name_from_title("Gstaldergeist || Élodie"),
            "Élodie"
        );
    }
}
