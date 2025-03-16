use super::data_grabber::TrashesSchedule;
use chrono::Datelike;
use teloxide::prelude::*;
use teloxide::{
    payloads::SendMessageSetters,
    types::{InlineKeyboardButton, InlineKeyboardMarkup},
};

async fn send(bot: &Bot, channel: i64, message: &str) {
    match bot.send_message(ChatId(channel), message).await {
        Ok(_) => println!("Scheduled message sent successfully"),
        Err(e) => eprintln!("Error sending scheduled message: {}", e),
    }
}

async fn weekly_update(bot: &Bot, config: &super::Config, schedule: &TrashesSchedule) {
    let global_chat_update_txt = format!("The new food master is {}.", schedule.master_name);
    send(bot, config.global_channel_id, &global_chat_update_txt).await;

    let mut master_update_txt = String::new();
    for i in 1..8 {
        // iterate over the next 7 days
        let date = chrono::Local::now().naive_local().date() + chrono::Duration::days(i);
        let trashes = schedule.dates.get(&date);
        match trashes {
            None => continue,
            Some(trashes) => {
                let trashes_str = trashes
                    .iter()
                    .fold(String::new(), |acc, trash| format!("{} {}", acc, trash));
                let day_update = format!("{} on {},\n", trashes_str, date.weekday());
                master_update_txt.push_str(&day_update);
            }
        }
    }
    let keyboard = InlineKeyboardMarkup::new(vec![
        // First row with two buttons
        vec![InlineKeyboardButton::callback(
            "Yes! Request new bags",
            "new_bags",
        )],
        vec![InlineKeyboardButton::callback(
            "We have enought bags!",
            "enough_bags",
        )],
    ]);

    let master_update_txt = format!(
        "Hello {}!\n\
        You are the new food master.\n\
        This week you need to put these trashes in front of the house before 7am.\n\
        Here is the schedule:\n\
        {}Do we need new we-recycle bags ?",
        schedule.master_name, master_update_txt
    );
    bot.send_message(ChatId(schedule.master_id), &master_update_txt)
        .reply_markup(keyboard)
        .await
        .unwrap();
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
            let trashes_str = trashes
                .iter()
                .fold(String::new(), |acc, trash| format!("{} {}", acc, trash));
            let daily_update_txt = format!(
                "Hello {} !\nDon't forget the {} trashes out before tomorrow morning! Have a nice evening!",
                schedule.master_name, trashes_str
            );

            let keyboard = InlineKeyboardMarkup::new(vec![
                // First row with two buttons
                vec![InlineKeyboardButton::callback("Done", "done")],
                vec![
                    InlineKeyboardButton::callback("Snooze", "snooze"),
                    InlineKeyboardButton::callback("I can't", "cant"),
                ],
            ]);

            let res = bot
                .send_message(ChatId(schedule.master_id), &daily_update_txt)
                .reply_markup(keyboard)
                .await;
            match res {
                Ok(_) => println!("Scheduled message sent successfully"),
                Err(e) => eprintln!("Error sending scheduled message: {}", e),
            }
            shared_task.lock().unwrap().state = super::TaskState::Pending;
        }
        None => {
            send(
                bot,
                schedule.master_id,
                &format!(
                    "Hi {}\nNo trashes tomorrow!\nHave a nice evening.",
                    schedule.master_name
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

pub async fn shame_update(bot: &Bot, config: &super::Config, schedule: &TrashesSchedule) {
    let tomorrow = chrono::Local::now().naive_local().date() + chrono::Duration::days(1);
    let trashes = schedule.dates.get(&tomorrow);

    if let Some(trashes) = trashes {
        let trashes_str = trashes
            .iter()
            .fold(String::new(), |acc, trash| format!("{} {}", acc, trash));

        let shame_update_txt = format!(
            "Unfortunately {} is not able to fulfill his role as Food master today!\n
            Could someone put the {} trashes out before tomorrow morning? Have a nice evening!",
            schedule.master_name, trashes_str
        );
        send(bot, config.global_channel_id, &shame_update_txt).await;
    }
}
