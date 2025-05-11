use chrono::{Datelike, TimeZone, Timelike, Weekday};
use std::env;
use std::error::Error;
use telegram_writer::{send_update, shame_update};
use teloxide::types::MaybeInaccessibleMessage;

use teloxide::prelude::*;
mod data_grabber;
mod email;
mod telegram_writer;

use teloxide::{
    payloads::SendMessageSetters,
    types::{InlineKeyboardButton, InlineKeyboardMarkup},
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TaskState {
    Failed,
    Pending,
    None,
}

pub struct SharedTaskState {
    state: TaskState,
    next_trigger: chrono::DateTime<chrono::Local>,
}

pub struct Config {
    pub flatmates: Vec<i64>,
    pub global_channel_id: i64,
    pub bot_token: String,
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

    Config {
        flatmates,
        global_channel_id: channel_id,
        bot_token,
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let app = config();
    println!("App config: {:?}", app.bot_token);
    let bot = Bot::new(&app.bot_token);
    let task_state = std::sync::Arc::new(std::sync::Mutex::new(SharedTaskState {
        state: TaskState::None,
        next_trigger: chrono::Local::now(),
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

async fn send_order_to_human(
    config: &Config,
    shared_task: std::sync::Arc<std::sync::Mutex<SharedTaskState>>,
    bot: &Bot,
    schedule: &data_grabber::TrashesSchedule,
) {
    send_update(
        bot,
        config,
        schedule,
        chrono::Local::now().weekday() == Weekday::Sun,
        shared_task,
    )
    .await;
}

async fn control_human_accomplishment(
    config: &Config,
    shared_task: std::sync::Arc<std::sync::Mutex<SharedTaskState>>,
    bot: &Bot,
    schedule: &data_grabber::TrashesSchedule,
) {
    if shared_task.lock().unwrap().state == TaskState::Pending {
        shame_update(bot, config, schedule).await;
        shared_task.lock().unwrap().state = TaskState::None;
    }
}

// Function to send messages on a schedule
async fn send_scheduled_messages(
    config: Config,
    shared_task: std::sync::Arc<std::sync::Mutex<SharedTaskState>>,
    bot: Bot,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    loop {
        let now = chrono::Local::now();
        let mut next_trigger = shared_task.lock().unwrap().next_trigger;
        if now < next_trigger {
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            continue;
        }
        let today = now.date_naive();
        let weekly = now.weekday() == chrono::Weekday::Sun;
        let until_date = if weekly {
            today + chrono::Duration::days(7)
        } else {
            today + chrono::Duration::days(1)
        };

        let trashes_schedule = data_grabber::get_trashes(&config, today, until_date).await;

        telegram_writer::send_update(
            &bot,
            &config,
            &trashes_schedule,
            weekly,
            shared_task.clone(),
        )
        .await;

        if next_trigger.hour() >= 21 {
            control_human_accomplishment(&config, shared_task.clone(), &bot, &trashes_schedule)
                .await;
            next_trigger = next_trigger + chrono::Duration::days(1);
            next_trigger = next_trigger.with_hour(18).unwrap();
        } else {
            send_order_to_human(&config, shared_task.clone(), &bot, &trashes_schedule).await;
            next_trigger = next_trigger.with_hour(21).unwrap();
        }
        shared_task.lock().unwrap().next_trigger = next_trigger;
    }
}

async fn handle_message(bot: Bot, msg: Message) -> ResponseResult<()> {
    println!("Received message: {:?} from {:?}", msg.text(), msg.chat.id);
    if let Some(text) = msg.text() {
        if text == "ping" {
            let keyboard = InlineKeyboardMarkup::new(vec![
                // First row with two buttons
                vec![InlineKeyboardButton::callback("AAA", "aaa")],
                vec![InlineKeyboardButton::callback("BBB", "bbb")],
            ]);

            bot.send_message(msg.chat.id, "pong!")
                .reply_markup(keyboard)
                .await?;
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
                    let _ = bot
                        .edit_message_text(
                            chat_id,
                            message.id(),
                            "Thank you! Have a nice evening. <3",
                        )
                        .await?;
                    let mut task_state = task_state.lock().unwrap();
                    if task_state.state != TaskState::Pending {
                        return Ok(());
                    }
                    task_state.state = TaskState::None;
                    // set next trigger to tomorrow 18:00
                    let tomorrow = chrono::Local::now() + chrono::Duration::days(1);
                    let tomorrow_evening = chrono::Local
                        .with_ymd_and_hms(
                            tomorrow.year(),
                            tomorrow.month(),
                            tomorrow.day(),
                            18,
                            00,
                            00,
                        )
                        .unwrap();
                    task_state.next_trigger = tomorrow_evening;
                }
                "cant" => {
                    // Handle "I can't" button
                    bot.edit_message_text(
                        chat_id,
                        message.id(),
                        "No problem. I will ask the others to help.",
                    )
                    .await?;
                    let mut task_state = task_state.lock().unwrap();
                    if task_state.state != TaskState::Pending {
                        return Ok(());
                    }
                    task_state.state = TaskState::Failed;
                    // set next trigger to now
                    task_state.next_trigger = chrono::Local::now();
                }
                "new_bags" => {
                    // Handle "No bags" button
                    let keyboard = InlineKeyboardMarkup::new(vec![
                        // First row with two buttons
                        vec![InlineKeyboardButton::callback("NEW BAGS !!!", "sure_bags")],
                        vec![InlineKeyboardButton::callback(
                            "Nah, no need",
                            "enough_bags",
                        )],
                    ]);

                    bot.edit_message_text(
                        chat_id,
                        message.id(),
                        "Are you sure ? A request will be sent to We-Recycle.",
                    )
                    .reply_markup(keyboard)
                    .await?;
                }
                "sure_bags" => {
                    // Handle "Sure bags" button
                    bot.edit_message_text(
                        chat_id,
                        message.id(),
                        "Thank you! I sent a request to We-Recycle.",
                    )
                    .await?;
                    email::request_new_bags();
                }
                "enough_bags" => {
                    // Handle "Enough bags" button
                    bot.edit_message_text(chat_id, message.id(), "Great! Have a nice evening.")
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
