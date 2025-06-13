use chrono::{Datelike, TimeZone, Timelike, Weekday};
use std::env;
use std::error::Error;
use telegram_writer::{send_update, shame_update};

use teloxide::prelude::*;
mod data_grabber;
mod email;
mod telegram_writer;
mod answer_handler;

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
        next_trigger: compute_next_trigger(),
    }));
    let app_state = dptree::deps![std::sync::Arc::clone(&task_state)];
    let scheduled_task = tokio::spawn(send_scheduled_messages(app, task_state, bot.clone()));

    let message_handler = Update::filter_message().endpoint(handle_message);
    let callback_handler = Update::filter_callback_query().endpoint(answer_handler::handle_callback_query);
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

fn compute_next_trigger(
) -> chrono::DateTime<chrono::Local> {
    let now = chrono::Local::now();
    if now.hour() < 18 {
        now.with_hour(18).unwrap()
    } else if now.hour() < 21 {
        now.with_hour(21).unwrap()
    } else {
        (now + chrono::Duration::days(1)).with_hour(18).unwrap()
    }
}

// Function to send messages on a schedule
async fn send_scheduled_messages(
    config: Config,
    shared_task: std::sync::Arc<std::sync::Mutex<SharedTaskState>>,
    bot: Bot,
) -> Result<(), String> {
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

        let trashes_schedule = data_grabber::get_trashes(&config, today, until_date).await.map_err(|e| e.to_string())?;
        if next_trigger.hour() >= 21 {
            control_human_accomplishment(&config, shared_task.clone(), &bot, &trashes_schedule)
                .await;
        } else {
            send_order_to_human(&config, shared_task.clone(), &bot, &trashes_schedule).await;
        }
        next_trigger = compute_next_trigger();
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
