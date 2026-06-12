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

/// Maps an Adliswil API waste-type code to a `TrashType`. Codes outside the
/// known set are collection types we don't track and yield `None`.
fn waste_type_from_code(code: i32) -> Option<TrashType> {
    match code {
        1 => Some(TrashType::Normal),
        2 => Some(TrashType::Bio),
        3 => Some(TrashType::Cardboard),
        4 => Some(TrashType::Paper),
        _ => None,
    }
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
                if naive > from
                    && naive <= to
                    && let Some(trash_type) = waste_type_from_code(event.waste_type)
                {
                    result.entry(naive).or_default().push(trash_type);
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

    #[test]
    fn maps_known_waste_codes() {
        let name = |code| waste_type_from_code(code).map(|t| t.to_string());
        assert_eq!(name(1).as_deref(), Some("Normal"));
        assert_eq!(name(2).as_deref(), Some("Bio"));
        assert_eq!(name(3).as_deref(), Some("Cardboard"));
        assert_eq!(name(4).as_deref(), Some("Paper"));
    }

    #[test]
    fn unknown_waste_codes_are_skipped() {
        assert!(waste_type_from_code(0).is_none());
        assert!(waste_type_from_code(5).is_none());
        assert!(waste_type_from_code(-1).is_none());
    }
}
