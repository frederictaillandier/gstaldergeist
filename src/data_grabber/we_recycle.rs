use super::TrashType;
use chrono::{self, Datelike, NaiveDate};
use regex::Regex;

use std::collections::HashMap;

pub struct WeRecycleWasteGrabber;

#[async_trait::async_trait]
impl super::WasteGrabber for WeRecycleWasteGrabber {
    async fn get_trashes(&self, from: NaiveDate, to: NaiveDate) -> Result<HashMap<NaiveDate, Vec<TrashType>>, String> {
        let extracted_dates = download_pdf().await.map_err(|err| err.to_string())?;
        let mut result = HashMap::new();
        for date in extracted_dates {
            if date > from && date <= to {
                result
                    .entry(date)
                    .or_insert_with(Vec::new)
                    .push(TrashType::WeRecycle);
            }
        }
        Ok(result)
    }
}

fn regex_caps_to_datetime(caps: &regex::Captures) -> Option<NaiveDate> {
    let date = &caps[1];

    if let Some(regions) = caps.get(3) {
        if regions.as_str().contains("19") {
            let current_year = chrono::Utc::now().date_naive().year();
            let naive_date =
                chrono::NaiveDate::parse_from_str(&format!("{}{}", date, current_year), "%d.%m.%Y")
                    .ok()?;
            return Some(naive_date);
        }
    }
    None
}

fn extract_dates_from_txt(text: String) -> Result<Vec<NaiveDate>, Box<dyn std::error::Error>> {
    let mut result = Vec::new();

    let date_pattern = r"(\d{1,2}\.\d{1,2}\.)";
    let weekday_pattern = r"([A-Z]{2})";
    let regions_pattern = r"([\d\s\+\-]+(?:\s+\d+\s*-\s*\d+)?(?:\s+\d+\s*-\s*\d+)*)?";
    let regex = format!(
        "{}\\s+{}\\s*{}?\\s+",
        date_pattern, weekday_pattern, regions_pattern
    );
    let re = Regex::new(&regex)?;

    for caps in re.captures_iter(&text) {
        if let Some(datetime) = regex_caps_to_datetime(&caps) {
            result.push(datetime);
        }
    }
    result.sort();
    Ok(result)
}

async fn download_pdf() -> Result<Vec<NaiveDate>, Box<dyn std::error::Error>> {
    let url = "https://www.werecycle.ch/en/abholdaten/";
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;
    let response = client.get(url).send().await?;
    let body = response.text().await?;
    //println!("{:?}", body);
    let regex = Regex::new(r#"href="([^"]+\d+.pdf)""#)?;
    let caps = regex.captures_iter(&body);

    let mut result = Vec::new();

    for cap in caps {
        println!("cap {:?}", cap);
        let pdf_url = cap.get(1).ok_or("pdf url corrupted")?.as_str();
        println!("pdf_url {:?}", pdf_url);
        let pdf = client.get(pdf_url).send().await?;
        let pdf_bytes = pdf.bytes().await?;
        let pdf_text = pdf_extract::extract_text_from_mem(&pdf_bytes)?;
        let dates = extract_dates_from_txt(pdf_text)?;

        result.extend(dates);
    }
    result.sort();
    Ok(result)
}
