use super::data_grabber::{TrashType, TrashesSchedule};
use chrono::{Datelike, NaiveDate};
use std::collections::HashMap;
use teloxide::prelude::*;
use teloxide::{
    payloads::SendMessageSetters,
    types::{InlineKeyboardButton, InlineKeyboardMarkup},
};

async fn send(bot: &Bot, channel: i64, message: &str) {
    match bot.send_message(ChatId(channel), message).await {
        Ok(_) => tracing::info!("Scheduled message sent successfully"),
        Err(e) => tracing::error!("Error sending scheduled message: {}", e),
    }
}

fn format_trashes(trashes: &[TrashType]) -> String {
    trashes
        .iter()
        .map(|trash| trash.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Lists the collection days in the seven days after `today`, one per line,
/// skipping days with no collection.
fn format_week_schedule(dates: &HashMap<NaiveDate, Vec<TrashType>>, today: NaiveDate) -> String {
    let mut schedule = String::new();
    for offset in 1..8 {
        let date = today + chrono::Duration::days(offset);
        if let Some(trashes) = dates.get(&date) {
            schedule.push_str(&format!("{} on {},\n", format_trashes(trashes), date.weekday()));
        }
    }
    schedule
}

pub async fn notify_group(bot: &Bot, config: &super::Config, message: &str) {
    send(bot, config.global_channel_id, message).await;
}

async fn weekly_update(bot: &Bot, config: &super::Config, schedule: &TrashesSchedule) {
    let global_chat_update_txt =
        format!("The new food master is {}.", schedule.tomorrow_master_name);
    send(bot, config.global_channel_id, &global_chat_update_txt).await;

    let today = chrono::Local::now().naive_local().date();
    let master_update_txt = format_week_schedule(&schedule.dates, today);
    let keyboard = InlineKeyboardMarkup::new(vec![
        // First row with two buttons
        vec![InlineKeyboardButton::callback(
            "Yes! Request new bags.",
            "new_bags",
        )],
        vec![InlineKeyboardButton::callback(
            "No. We have enough bags.",
            "enough_bags",
        )],
    ]);

    let master_update_txt = format!(
        "Hello {}!\n\
        You are the new food master.\n\
        This week you need to put these trashes in front of the house before 7am.\n\
        Here is the schedule:\n\
        {}",
        schedule.tomorrow_master_name, master_update_txt
    );
    send(bot, schedule.tomorrow_master_id, &master_update_txt).await;

    let request_bags_txt =
        format!("Can you look if we still have enough We-Recycle bags? Do we need to order new?");
    match bot
        .send_message(ChatId(schedule.tomorrow_master_id), &request_bags_txt)
        .reply_markup(keyboard)
        .await
    {
        Ok(_) => tracing::info!("Scheduled message sent successfully"),
        Err(e) => tracing::error!("Error sending scheduled message: {}", e),
    }
}

async fn daily_update(
    bot: &Bot,
    _config: &super::Config,
    schedule: &TrashesSchedule,
    shared_task: std::sync::Arc<std::sync::Mutex<super::SharedTaskState>>,
) {
    let tomorrow = chrono::Local::now().naive_local().date() + chrono::Duration::days(1);
    let trashes = schedule.dates.get(&tomorrow);
    match trashes {
        Some(trashes) => {
            let daily_update_txt = format!(
                "Hello {} !\nDon't forget to put the {} trashes out before tomorrow morning! \n\
                If you don't answer this message before 9pm,\n\
                a reminder will be sent to all the flatmates.\n\
                ",
                schedule.tomorrow_master_name,
                format_trashes(trashes)
            );

            let keyboard = InlineKeyboardMarkup::new(vec![
                // First row with two buttons
                vec![InlineKeyboardButton::callback("Done", "done")],
                vec![InlineKeyboardButton::callback("I can't", "cant")],
            ]);

            let res = bot
                .send_message(ChatId(schedule.tomorrow_master_id), &daily_update_txt)
                .reply_markup(keyboard)
                .await;
            match res {
                Ok(_) => tracing::info!("Scheduled message sent successfully"),
                Err(e) => tracing::error!("Error sending scheduled message: {}", e),
            }
            shared_task.lock().unwrap().state = super::TaskState::Pending;
        }
        None => {
            send(
                bot,
                schedule.tomorrow_master_id,
                &format!(
                    "Hi {}\nNo trashes tomorrow!\nHave a nice evening.",
                    schedule.tomorrow_master_name
                ),
            )
            .await;
            shared_task.lock().unwrap().state = super::TaskState::None;
        }
    }
}

pub async fn send_update(
    bot: &Bot,
    config: &super::Config,
    schedule: &TrashesSchedule,
    weekly: bool,
    shared_task: std::sync::Arc<std::sync::Mutex<super::SharedTaskState>>,
) {
    if weekly {
        weekly_update(bot, config, schedule).await;
    }
    daily_update(bot, config, schedule, shared_task).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_a_single_trash() {
        assert_eq!(format_trashes(&[TrashType::Normal]), "Normal");
    }

    #[test]
    fn space_separates_multiple_trashes_without_leading_space() {
        assert_eq!(
            format_trashes(&[TrashType::Normal, TrashType::Bio, TrashType::Paper]),
            "Normal Bio Paper"
        );
    }

    #[test]
    fn empty_slice_yields_empty_string() {
        assert_eq!(format_trashes(&[]), "");
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn week_schedule_lists_collection_days_in_order() {
        // 2024-01-01 is a Monday, so offsets 1..8 cover Tue..next Mon.
        let mut dates = HashMap::new();
        dates.insert(date(2024, 1, 4), vec![TrashType::Bio, TrashType::Paper]);
        dates.insert(date(2024, 1, 2), vec![TrashType::Normal]);
        assert_eq!(
            format_week_schedule(&dates, date(2024, 1, 1)),
            "Normal on Tue,\nBio Paper on Thu,\n"
        );
    }

    #[test]
    fn week_schedule_is_empty_without_collections() {
        let dates = HashMap::new();
        assert_eq!(format_week_schedule(&dates, date(2024, 1, 1)), "");
    }

    #[test]
    fn week_schedule_excludes_today_and_the_eighth_day() {
        let mut dates = HashMap::new();
        dates.insert(date(2024, 1, 1), vec![TrashType::Normal]); // today, offset 0
        dates.insert(date(2024, 1, 9), vec![TrashType::Paper]); // offset 8
        assert_eq!(format_week_schedule(&dates, date(2024, 1, 1)), "");
    }
}

pub async fn shame_update(bot: &Bot, config: &super::Config, schedule: &TrashesSchedule) {
    let tomorrow = chrono::Local::now().naive_local().date() + chrono::Duration::days(1);
    let trashes = schedule.dates.get(&tomorrow);

    if let Some(trashes) = trashes {
        let shame_update_txt = format!(
            "Unfortunately {} is not able to fulfill his role as Food master today...Could someone put the {} trashes out before tomorrow morning? Have a nice evening!",
            schedule.tomorrow_master_name,
            format_trashes(trashes)
        );
        send(bot, config.global_channel_id, &shame_update_txt).await;
    }
}
