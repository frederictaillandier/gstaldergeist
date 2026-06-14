use super::TrashType;
use crate::error::GstaldergeistError;
use chrono::{DateTime, Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

/// The Adliswil calendar API is queried one month at a time. Returns every
/// `(year, month)` pair the `[from, to]` window touches so that windows
/// spanning a month (or year) boundary fetch all the relevant months.
fn months_in_range(from: NaiveDate, to: NaiveDate) -> Vec<(i32, u32)> {
    let mut months = Vec::new();
    let (mut year, mut month) = (from.year(), from.month());
    while (year, month) <= (to.year(), to.month()) {
        months.push((year, month));
        if month == 12 {
            year += 1;
            month = 1;
        } else {
            month += 1;
        }
    }
    months
}

#[async_trait::async_trait]
impl super::WasteGrabber for AdliswilWasteGrabber {
    async fn get_trashes(
        &self,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<HashMap<NaiveDate, Vec<TrashType>>, GstaldergeistError> {
        let client = reqwest::Client::new();

        let mut result: HashMap<NaiveDate, Vec<TrashType>> = HashMap::new();
        for (year, month) in months_in_range(from, to) {
            let url = format!(
                "https://adliswil.entsorglos.swiss/backend/widget/calendar-dates/{:02}-{}/",
                month, year
            );

            let response = client.get(url).send().await?;

            let waste_json = response.text().await?;
            let wastes: AdliswilWaste = serde_json::from_str(&waste_json)?;

            for event in wastes.results.events {
                let naive = event.date.date_naive();
                if super::is_in_collection_window(naive, from, to) {
                    let trastype = match event.waste_type {
                        1 => super::TrashType::Normal,
                        2 => super::TrashType::Bio,
                        3 => super::TrashType::Cardboard,
                        4 => super::TrashType::Paper,
                        _ => continue,
                    };
                    result.entry(naive).or_default().push(trastype);
                }
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    #[test]
    fn single_month_window() {
        assert_eq!(
            months_in_range(date(2026, 1, 5), date(2026, 1, 20)),
            vec![(2026, 1)]
        );
    }

    #[test]
    fn window_spanning_two_months() {
        assert_eq!(
            months_in_range(date(2026, 1, 28), date(2026, 2, 4)),
            vec![(2026, 1), (2026, 2)]
        );
    }

    #[test]
    fn window_spanning_year_boundary() {
        assert_eq!(
            months_in_range(date(2025, 12, 28), date(2026, 1, 4)),
            vec![(2025, 12), (2026, 1)]
        );
    }

    #[test]
    fn from_after_to_yields_no_months() {
        assert_eq!(
            months_in_range(date(2026, 2, 1), date(2026, 1, 1)),
            Vec::<(i32, u32)>::new()
        );
    }
}
