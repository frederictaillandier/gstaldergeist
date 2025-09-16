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
    pub _master_name: String,
    pub _master_id: i64,
    pub tomorrow_master_name: String,
    pub tomorrow_master_id: i64,
}

async fn tomorrow_food_master_id(config: &super::Config) -> i64 {
    let tomorrow = chrono::Local::now().naive_local().date() + chrono::Duration::days(1);

    let chat_id =
        config.flatmates[(1 + tomorrow.iso_week().week0() as usize) % config.flatmates.len()];
    chat_id
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

async fn today_food_master_id(config: &super::Config) -> i64 {
    let chat_id = config.flatmates
        [(1 + chrono::Local::now().iso_week().week0() as usize) % config.flatmates.len()];
    chat_id
}

async fn grab_today_food_master_name(config: &super::Config) -> String {
    let client = reqwest::Client::new();

    let bot_token = &config.bot_token;
    let chat_id = &config.flatmates
        [(1 + chrono::Local::now().iso_week().week0() as usize) % config.flatmates.len()];

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
        _master_name: grab_today_food_master_name(config).await,
        _master_id: today_food_master_id(config).await,
        tomorrow_master_name: grab_tomorrow_food_master_name(config).await,
        tomorrow_master_id: tomorrow_food_master_id(config).await,
    })
}
