use chrono::{Datelike, TimeZone};
use std::env;
use std::error::Error;
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
    reminders_sent: u32,
}

pub struct Config {
    pub flatmates: Vec<i64>,
    pub global_channel_id: i64,
    pub bot_token: String,
    pub immediate: bool,
    pub force_weekly: bool,
    pub delta_reminder_sec: i64,
    pub nb_reminders: u32,
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
    let delta_reminder_sec = env::var("DELTA_REMINDER_SEC")
        .expect("DELTA_REMINDER_SEC not set")
        .parse()
        .expect("DELTA_REMINDER_SEC must be a number");
    let nb_reminders = env::var("NB_REMINDERS")
        .expect("NB_REMINDERS not set")
        .parse()
        .expect("NB_REMINDERS must be a number");

    Config {
        flatmates,
        global_channel_id: channel_id,
        bot_token,
        immediate,
        force_weekly,
        delta_reminder_sec,
        nb_reminders,
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
        reminders_sent: 0,
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

// Function to send messages on a schedule
async fn send_scheduled_messages(
    config: Config,
    shared_task: std::sync::Arc<std::sync::Mutex<SharedTaskState>>,
    bot: Bot,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    loop {
        let now = chrono::Local::now();
        let next_trigger = shared_task.lock().unwrap().next_trigger;
        println!(
            "tick now : {:?} next trigger: {:?}",
            now.to_rfc2822(),
            next_trigger.to_rfc2822()
        );

        if now < next_trigger {
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            continue;
        }

        let today = chrono::Local::now().date_naive();
        let weekly = (today.weekday().number_from_monday() == 7
            && shared_task.lock().unwrap().state == TaskState::None)
            || config.force_weekly;
        let until_date = if weekly {
            today + chrono::Duration::days(7)
        } else {
            today + chrono::Duration::days(1)
        };

        let trashes_schedule = data_grabber::get_trashes(&config, today, until_date).await;
        if shared_task.lock().unwrap().reminders_sent >= config.nb_reminders
            || shared_task.lock().unwrap().state == TaskState::Failed
        {
            telegram_writer::shame_update(&bot, &config, &trashes_schedule).await;
            shared_task.lock().unwrap().state = TaskState::Failed;
        } else {
            telegram_writer::send_update(
                &bot,
                &config,
                &trashes_schedule,
                weekly,
                shared_task.clone(),
            )
            .await;
        }
        // Update the next trigger time
        let mut shared_task = shared_task.lock().unwrap();
        match shared_task.state {
            TaskState::None => {
                if trashes_schedule.dates.is_empty() {
                    shared_task.state = TaskState::None;
                    shared_task.reminders_sent = 0;
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
                    shared_task.next_trigger = tomorrow_evening;
                } else {
                    shared_task.state = TaskState::Pending;
                    shared_task.next_trigger =
                        now + chrono::Duration::seconds(config.delta_reminder_sec);
                }
            }
            TaskState::Pending => {
                shared_task.state = TaskState::Pending;
                shared_task.next_trigger =
                    now + chrono::Duration::seconds(config.delta_reminder_sec);
                shared_task.reminders_sent += 1;
            }
            TaskState::Failed => {
                shared_task.state = TaskState::None;
                shared_task.reminders_sent = 0;
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

                shared_task.next_trigger = tomorrow_evening;
            }
        }
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
                    task_state.reminders_sent = 0;
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
                    task_state.reminders_sent = 0;
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
                "aaa" => {
                    // Handle "AAA" button
                    bot.edit_message_text(chat_id, message.id(), "You clicked AAA!")
                        .await?;
                }
                "bbb" => {
                    // Handle "BBB" button
                    bot.edit_message_text(chat_id, message.id(), "You clicked BBB!")
                        .await?;
                    email::request_new_bags();
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
