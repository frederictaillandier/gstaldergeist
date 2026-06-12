use super::TrashType;
use crate::error::GstaldergeistError;
use chrono::{self, Datelike, NaiveDate};
use lopdf::Document;
use regex::Regex;
use std::collections::HashMap;

pub struct WeRecycleWasteGrabber;

/// The flat's We-Recycle collection region.
const COLLECTION_REGION: u32 = 19;

/// True if `regions` lists `target` as a standalone number. The regions column
/// is a space/`+`/`-` separated list of region numbers, so we compare whole
/// tokens rather than substrings — otherwise `190` or `119` would falsely match
/// region `19`.
fn regions_include(regions: &str, target: u32) -> bool {
    regions
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|token| token.parse::<u32>().ok())
        .any(|region| region == target)
}

#[async_trait::async_trait]
impl super::WasteGrabber for WeRecycleWasteGrabber {
    async fn get_trashes(
        &self,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<HashMap<NaiveDate, Vec<TrashType>>, GstaldergeistError> {
        let extracted_dates = download_pdf().await?;
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
        if regions_include(regions.as_str(), COLLECTION_REGION) {
            let current_year = chrono::Utc::now().date_naive().year();
            let naive_date =
                chrono::NaiveDate::parse_from_str(&format!("{}{}", date, current_year), "%d.%m.%Y")
                    .ok()?;
            return Some(naive_date);
        }
    }
    None
}

fn extract_dates_from_txt(text: String) -> Result<Vec<NaiveDate>, GstaldergeistError> {
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

fn extract_text_with_lopdf(pdf_bytes: &[u8]) -> Result<String, GstaldergeistError> {
    let doc =
        Document::load_mem(pdf_bytes).map_err(|e| GstaldergeistError::PdfExtract(e.to_string()))?;
    let pages = doc.get_pages().keys().cloned().collect::<Vec<_>>();
    let pdf_text = doc
        .extract_text(&pages)
        .map_err(|e| GstaldergeistError::PdfExtract(e.to_string()))?;
    Ok(pdf_text)
}

async fn download_pdf() -> Result<Vec<NaiveDate>, GstaldergeistError> {
    let url = "https://www.werecycle.ch/en/abholdaten/";
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;
    let response = client.get(url).send().await?;
    let body = response.text().await?;
    let regex = Regex::new(r#"href="([^"]+\d+.pdf)""#)?;
    let caps = regex.captures_iter(&body);

    let mut result = Vec::new();

    for cap in caps {
        let pdf_url = cap
            .get(1)
            .ok_or(GstaldergeistError::Other("pdf url corrupted".to_string()))?
            .as_str();
        let pdf = client.get(pdf_url).send().await?;
        let pdf_bytes = pdf.bytes().await?;
        let pdf_text = extract_text_with_lopdf(&pdf_bytes)?;
        let dates = extract_dates_from_txt(pdf_text)?;

        result.extend(dates);
    }
    result.sort();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The PDF dates carry no year, so `extract_dates_from_txt` stamps them with
    /// the current year. Tests build their expectations against the same year.
    fn date(day: u32, month: u32) -> NaiveDate {
        let year = chrono::Utc::now().date_naive().year();
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    #[test]
    fn keeps_only_collection_dates_for_region_19() {
        // The flat sits in collection region 19, so only rows whose region
        // column mentions "19" are relevant.
        let dates = extract_dates_from_txt("05.06. FR 19 ".to_string()).unwrap();
        assert_eq!(dates, vec![date(5, 6)]);
    }

    #[test]
    fn ignores_rows_for_other_regions() {
        let dates = extract_dates_from_txt("01.01. MO 20 ".to_string()).unwrap();
        assert!(dates.is_empty());
    }

    #[test]
    fn matches_region_19_within_a_range_of_regions() {
        // Region 19 can appear among several space/plus/dash separated regions.
        let dates = extract_dates_from_txt("12.07. SA 7 + 19 - 20 ".to_string()).unwrap();
        assert_eq!(dates, vec![date(12, 7)]);
    }

    #[test]
    fn ignores_region_numbers_that_merely_contain_19() {
        // "190" and "119" contain "19" as a substring but are not region 19.
        assert!(extract_dates_from_txt("05.06. FR 190 ".to_string())
            .unwrap()
            .is_empty());
        assert!(extract_dates_from_txt("05.06. FR 119 ".to_string())
            .unwrap()
            .is_empty());
    }

    #[test]
    fn ignores_rows_without_a_region_column() {
        let dates = extract_dates_from_txt("05.06. FR ".to_string()).unwrap();
        assert!(dates.is_empty());
    }

    #[test]
    fn returns_dates_sorted_ascending() {
        let text = "12.07. SA 19 \n05.06. FR 19 \n09.06. TU 5 + 19 ".to_string();
        let dates = extract_dates_from_txt(text).unwrap();
        assert_eq!(dates, vec![date(5, 6), date(9, 6), date(12, 7)]);
    }

    #[test]
    fn returns_empty_when_no_dates_present() {
        let dates = extract_dates_from_txt("no dates in this text".to_string()).unwrap();
        assert!(dates.is_empty());
    }
}
