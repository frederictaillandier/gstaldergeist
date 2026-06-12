use super::data_grabber::{TrashType, TrashesSchedule};
use chrono::Datelike;
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

async fn send_with_keyboard(
    bot: &Bot,
    channel: i64,
    message: &str,
    keyboard: InlineKeyboardMarkup,
) {
    match bot
        .send_message(ChatId(channel), message)
        .reply_markup(keyboard)
        .await
    {
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

pub async fn notify_group(bot: &Bot, config: &super::Config, message: &str) {
    send(bot, config.global_channel_id, message).await;
}

async fn weekly_update(bot: &Bot, config: &super::Config, schedule: &TrashesSchedule) {
    let global_chat_update_txt =
        format!("The new food master is {}.", schedule.tomorrow_master_name);
    send(bot, config.global_channel_id, &global_chat_update_txt).await;

    let mut master_update_txt = String::new();
    for i in 1..8 {
        // iterate over the next 7 days
        let date = chrono::Local::now().naive_local().date() + chrono::Duration::days(i);
        let trashes = schedule.dates.get(&date);
        match trashes {
            None => continue,
            Some(trashes) => {
                let day_update =
                    format!("{} on {},\n", format_trashes(trashes), date.weekday());
                master_update_txt.push_str(&day_update);
            }
        }
    }
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
        "Can you look if we still have enough We-Recycle bags? Do we need to order new?";
    send_with_keyboard(bot, schedule.tomorrow_master_id, request_bags_txt, keyboard).await;
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

            send_with_keyboard(
                bot,
                schedule.tomorrow_master_id,
                &daily_update_txt,
                keyboard,
            )
            .await;
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
