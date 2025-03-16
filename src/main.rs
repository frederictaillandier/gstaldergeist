use chrono::{Datelike, Timelike};
use std::env;
use std::error::Error;

use teloxide::prelude::*;
mod data_grabber;
mod telegram_writer;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TaskState {
    Pending,
    Failed,
    None,
}

pub struct SharedTaskState {
    state: TaskState,
}

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
    let task_state = std::sync::Arc::new(std::sync::Mutex::new(SharedTaskState {
        state: TaskState::None,
    }));
    let app_state = dptree::deps![std::sync::Arc::clone(&task_state)];
    let scheduled_task = tokio::spawn(send_scheduled_messages(app, task_state, bot.clone()));

    let message_handler = Update::filter_message().endpoint(handle_message);
    let callback_handler = Update::filter_callback_query().endpoint(handle_callback_query);
    let handler = dptree::entry()
        .branch(message_handler)
        .branch(callback_handler);

    Dispatcher::builder(bot, handler)
        .dependencies(app_state)
        .build()
        .dispatch()
        .await;
    if let Err(e) = scheduled_task.await {
        eprintln!("Scheduled task failed: {}", e);
    }

    Ok(())
}

async fn wait_next_hour() {
    let now = chrono::Local::now();
    let next_hour = now + chrono::Duration::hours(1);

    let duration_to_trigger = next_hour.signed_duration_since(now);
    let interval = duration_to_trigger.to_std().unwrap();
    tokio::time::sleep(interval).await;
}

// Function to send messages on a schedule
async fn send_scheduled_messages(
    config: Config,
    shared_task: std::sync::Arc<std::sync::Mutex<SharedTaskState>>,
    bot: Bot,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    loop {
        if !config.immediate {
            wait_next_hour().await;
        } else {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }

        let now = chrono::Local::now();
        let today = chrono::Local::now().date_naive();
        let weekly = today.weekday().number_from_monday() == 7 || config.force_weekly;
        let until_date = if weekly {
            today + chrono::Duration::days(7)
        } else {
            today + chrono::Duration::days(1)
        };

        let current_task_state = shared_task.clone().lock().unwrap().state.clone();
        if now.hour() == 18 {
            let trashes_schedule = data_grabber::get_trashes(&config, today, until_date).await;
            telegram_writer::send_update(
                &bot,
                &config,
                &trashes_schedule,
                weekly,
                shared_task.clone(),
            )
            .await;
        } else if (current_task_state == TaskState::Pending && now.hour() >= 22)
            || current_task_state == TaskState::Failed
        {
            let trashes_schedule = data_grabber::get_trashes(&config, today, until_date).await;
            telegram_writer::shame_update(&bot, &config, &trashes_schedule).await;
            shared_task.lock().unwrap().state = TaskState::None;
        } else if current_task_state == TaskState::Pending && now.hour() >= 19 {
            let trashes_schedule = data_grabber::get_trashes(&config, today, until_date).await;
            telegram_writer::send_update(
                &bot,
                &config,
                &trashes_schedule,
                false,
                shared_task.clone(),
            )
            .await;
        }
    }
}

async fn handle_message(bot: Bot, msg: Message) -> ResponseResult<()> {
    println!("Received message: {:?} from {:?}", msg.text(), msg.chat.id);
    if let Some(text) = msg.text() {
        if text == "ping" {
            bot.send_message(msg.chat.id, "pong!").await?;
        }
    }
    Ok(())
}

// Add this new handler function for callback queries
async fn handle_callback_query(
    bot: Bot,
    query: CallbackQuery,
    task_state: std::sync::Arc<std::sync::Mutex<SharedTaskState>>,
) -> ResponseResult<()> {
    // Extract the callback data from the query
    if let Some(data) = &query.data {
        // Get the chat ID from the message
        if let Some(message) = query.message {
            let chat_id = message.chat().id;

            match data.as_str() {
                "done" => {
                    // Handle "Done" button
                    task_state.lock().unwrap().state = TaskState::None;
                    bot.send_message(chat_id, "Thank you <3").await?;
                }
                "snooze" => {
                    // Handle "Snooze" button
                    task_state.lock().unwrap().state = TaskState::Pending;
                    bot.send_message(chat_id, "Ok :) I'll remind you later.")
                        .await?;
                    // You might want to add logic to reschedule the reminder
                }
                "cant" => {
                    // Handle "I can't" button
                    task_state.lock().unwrap().state = TaskState::None;
                    bot.send_message(chat_id, "No problem. I will ask the others to help.")
                        .await?;
                }
                _ => {
                    // Handle unknown callback data
                    bot.send_message(chat_id, "Unrecognized option.").await?;
                }
            }

            // Answer the callback query to remove the "loading" state
            bot.answer_callback_query(query.id).await?;
        }
    }

    Ok(())
}
