use chrono::{Datelike, TimeZone, Timelike};
use std::env;
use std::error::Error;
use teloxide::prelude::*;
use teloxide::types::ChatId;
use tokio::time::interval;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let bot_token = env::var("TELEGRAM_BOT_TOKEN").expect("TELEGRAM_BOT_TOKEN not set");
    let channel_id_str = env::var("TELEGRAM_CHANNEL_ID").expect("TELEGRAM_CHANNEL_ID not set");
    let channel_id: i64 = channel_id_str
        .parse()
        .expect("TELEGRAM_CHANNEL_ID must be a number");

    let bot = Bot::new(bot_token);
    let scheduled_task = tokio::spawn(send_scheduled_messages(bot.clone(), ChatId(channel_id)));

    let handler = dptree::entry().branch(Update::filter_message().endpoint(handle_message));
    Dispatcher::builder(bot, handler).build().dispatch().await;
    if let Err(e) = scheduled_task.await {
        eprintln!("Scheduled task failed: {}", e);
    }

    Ok(())
}

async fn wait_to_trigger_hour() {
    let trigger_time = chrono::NaiveTime::from_hms_opt(12, 5, 0).unwrap();
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
    bot: Bot,
    chat_id: ChatId,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    loop {
        wait_to_trigger_hour().await;

        match bot.send_message(chat_id, "Hello World!").await {
            Ok(_) => println!("Scheduled message sent successfully"),
            Err(e) => eprintln!("Error sending scheduled message: {}", e),
        }
    }
}

async fn handle_message(bot: Bot, msg: Message) -> ResponseResult<()> {
    println!("Received message: {:?}", msg.text());
    // If someone sends the /start command, respond
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
