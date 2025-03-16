mod adliswil;
mod we_recycle;
use chrono::{Datelike, NaiveDate};
use core::fmt;

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

#[derive(Deserialize, Debug)]
struct ChatResult {
    result: ChatInfo,
}

#[derive(Deserialize, Debug)]
struct ChatInfo {
    title: String,
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
    pub master_name: String,
    pub master_id: i64,
}

async fn current_food_master_id(config: &super::Config) -> i64 {
    let chat_id = config.flatmates
        [2 + chrono::Local::now().iso_week().week0() as usize % config.flatmates.len()];
    chat_id
}

async fn grab_current_food_master_name(config: &super::Config) -> String {
    let client = reqwest::Client::new();

    let bot_token = &config.bot_token;
    let chat_id = &config.flatmates
        [2 + chrono::Local::now().iso_week().week0() as usize % config.flatmates.len()];

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
) -> TrashesSchedule {
    let mut dates = adliswil::get_trashes(from, to).await;
    let mut we_recycle = we_recycle::get_trashes(from, to).await;

    println!("{:?}", we_recycle);

    for (date, trashes) in we_recycle.drain() {
        dates.entry(date).or_default().extend(trashes);
    }

    TrashesSchedule {
        dates,
        master_name: grab_current_food_master_name(config).await,
        master_id: current_food_master_id(config).await,
    }
}
