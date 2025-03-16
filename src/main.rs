use chrono::{Datelike, TimeZone, Timelike};
use std::env;
use std::error::Error;

use teloxide::prelude::*;
mod data_grabber;
mod telegram_writer;

pub struct Config {
    pub flatmates: Vec<i64>,
    pub global_channel_id: i64,
    pub bot_token: String,
    pub immediate: bool,
    pub force_weekly: bool,
}

fn config() -> Config {
    let bot_token = env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not set");
    let channel_id_str = env::var("TELEGRAM_CHANNEL_ID").expect("TELEGRAM_CHANNEL_ID not set");
    let channel_id: i64 = channel_id_str
        .parse()
        .expect("TELEGRAM_CHANNEL_ID must be a number");
    let flatmates: String = env::var("TELEGRAM_FLATMATES").expect("TELEGRAM_FLATMATES not set");
    let flatmates: Vec<i64> = flatmates
        .split(',')
        .map(|s| {
            s.trim().parse().expect(
                "TELEGRAM_FLATMATES must be a comma-separated list of numbers like 123,456,789",
            )
        })
        .collect();

    let immediate = env::args().any(|arg| arg == "--immediate"); // used for test
    let force_weekly = env::args().any(|arg| arg == "--force-weekly"); // used for test

    Config {
        flatmates,
        global_channel_id: channel_id,
        bot_token,
        immediate,
        force_weekly,
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let app = config();
    let bot = Bot::new(&app.bot_token);
    let scheduled_task = tokio::spawn(send_scheduled_messages(app, bot.clone()));

    let handler = dptree::entry().branch(Update::filter_message().endpoint(handle_message));
    Dispatcher::builder(bot, handler).build().dispatch().await;
    if let Err(e) = scheduled_task.await {
        eprintln!("Scheduled task failed: {}", e);
    }

    Ok(())
}

async fn wait_to_trigger_hour() {
    let trigger_time = chrono::NaiveTime::from_hms_opt(18, 00, 0).unwrap();

    let now = chrono::Local::now();
    let next_notif_day = if now.time() >= trigger_time {
        now + chrono::Duration::days(1)
    } else {
        now
    };
    let next_notif_time = chrono::Local
        .with_ymd_and_hms(
            next_notif_day.year(),
            next_notif_day.month(),
            next_notif_day.day(),
            trigger_time.hour(),
            trigger_time.minute(),
            trigger_time.second(),
        )
        .unwrap();
    let duration_to_trigger = next_notif_time.signed_duration_since(now);
    let interval = duration_to_trigger.to_std().unwrap();
    tokio::time::sleep(interval).await;
}

// Function to send messages on a schedule
async fn send_scheduled_messages(
    app: Config,
    bot: Bot,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    loop {
        if !app.immediate {
            wait_to_trigger_hour().await;
        }

        let today = chrono::Local::now().date_naive();
        let weekly = today.weekday().number_from_monday() == 7 || app.force_weekly;
        let until_date = if weekly {
            today + chrono::Duration::days(7)
        } else {
            today + chrono::Duration::days(1)
        };

        let trashes_schedule = data_grabber::get_trashes(&app, today, until_date).await;
        telegram_writer::send_update(&bot, &app, &trashes_schedule, weekly).await;
    }
}

async fn handle_message(bot: Bot, msg: Message) -> ResponseResult<()> {
    println!("Received message: {:?} from {:?}", msg.text(), msg.chat.id);
    if let Some(text) = msg.text() {
        if text == "/start" {
            bot.send_message(
                msg.chat.id,
                "Bot is running! I'll send hourly messages to the configured channel.",
            )
            .await?;
        }
    }
    Ok(())
}
