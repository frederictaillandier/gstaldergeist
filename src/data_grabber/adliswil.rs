use crate::error::GstaldergeistError;
use chrono::{DateTime, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use super::TrashType;

#[derive(Serialize, Deserialize, Debug)]
struct Event {
    date: DateTime<chrono::Utc>,
    waste_type: i32,
}

#[derive(Serialize, Deserialize, Debug)]
struct AdliswilWasteInfo {
    events: Vec<Event>,
}

#[derive(Serialize, Deserialize, Debug)]
struct AdliswilWaste {
    results: AdliswilWasteInfo,
}

pub struct AdliswilWasteGrabber;

#[async_trait::async_trait]
impl super::WasteGrabber for AdliswilWasteGrabber {
    async fn get_trashes(
        &self,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<HashMap<NaiveDate, Vec<TrashType>>, GstaldergeistError> {
        let client = reqwest::Client::new();

        let url = format!(
            "https://adliswil.entsorglos.swiss/backend/widget/calendar-dates/{}/",
            from.format("%m-%Y")
        );

        let response = client.get(url).send().await?;

        let waste_json = response.text().await?;
        let wastes: AdliswilWaste = serde_json::from_str(&waste_json)?;

        let mut result: HashMap<NaiveDate, Vec<TrashType>> = HashMap::new();
        for event in wastes.results.events {
            let naive = event.date.date_naive();
            if naive > from && naive <= to {
                let trastype = match event.waste_type {
                    1 => super::TrashType::Normal,
                    2 => super::TrashType::Bio,
                    3 => super::TrashType::Cardboard,
                    4 => super::TrashType::Paper,
                    _ => continue,
                };
                result
                    .entry(event.date.date_naive())
                    .or_default()
                    .push(trastype);
            }
        }
        Ok(result)
    }
}
