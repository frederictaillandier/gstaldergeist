use super::data_grabber::TrashesSchedule;
use chrono::Datelike;
use teloxide::prelude::*;

async fn send(bot: &Bot, channel: i64, message: &str) {
    match bot.send_message(ChatId(channel), message).await {
        Ok(_) => println!("Scheduled message sent successfully"),
        Err(e) => eprintln!("Error sending scheduled message: {}", e),
    }
}

async fn weekly_update(bot: &Bot, config: &super::Config, schedule: &TrashesSchedule) {
    let global_chat_update_txt = format!("The new food master is {}.", schedule.master);
    send(bot, config.global_channel_id, &global_chat_update_txt).await;

    let mut master_update_txt = String::new();
    for i in 1..8 {
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
    let master_update_txt = format!(
        "Hello {}!\n\
        You are the new food master.\n\
        This week you need to put these trashes in front of the house before 7am.\n\
        Here is the schedule:\n\
        {}Have a nice evening!",
        schedule.master, master_update_txt
    );
    send(bot, config.flatmates[1], &master_update_txt).await;
}

async fn daily_update(bot: &Bot, config: &super::Config, schedule: &TrashesSchedule) {
    let tomorrow = chrono::Local::now().naive_local().date() + chrono::Duration::days(1);
    let trashes = schedule.dates.get(&tomorrow);
    match trashes {
        Some(trashes) => {
            let trashes_str = trashes
                .iter()
                .fold(String::new(), |acc, trash| format!("{} {}", acc, trash));
            let daily_update_txt = format!(
                "Hello {} !\nDon't forget the {} trashes out before tomorrow morning! Have a nice evening!",
                schedule.master, trashes_str
            );
            send(bot, config.flatmates[1], &daily_update_txt).await;
        }
        None => {
            send(
                bot,
                config.flatmates[1],
                &format!(
                    "Hi {}\nNo trashes tomorrow!\nHave a nice evening.",
                    schedule.master
                ),
            )
            .await;
        }
    }
}

pub async fn send_update(
    bot: &Bot,
    config: &super::Config,
    schedule: &TrashesSchedule,
    weekly: bool,
) {
    if weekly {
        weekly_update(bot, config, schedule).await;
    }
    daily_update(bot, config, schedule).await;
}
