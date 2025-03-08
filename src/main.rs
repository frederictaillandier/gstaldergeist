use std::env;
use std::error::Error;
use std::time::Duration;
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

// Function to send messages on a schedule
async fn send_scheduled_messages(
    bot: Bot,
    chat_id: ChatId,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut interval = interval(Duration::from_secs(10));

    loop {
        interval.tick().await;
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
