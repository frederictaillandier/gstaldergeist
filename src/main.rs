use chrono::{Datelike, TimeZone, Timelike, Weekday};
use std::env;
use telegram_writer::{send_update, shame_update};

use teloxide::prelude::*;
mod answer_handler;
mod data_grabber;
mod database;
mod email;
mod error;
mod telegram_writer;

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

fn config() -> Result<Config, error::GstaldergeistError> {
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

    Ok(Config {
        flatmates,
        global_channel_id: channel_id,
        bot_token,
    })
}

#[tokio::main]
async fn main() -> Result<(), error::GstaldergeistError> {
    let app = config()?;
    tracing::subscriber::set_global_default(
        tracing_subscriber::fmt::Subscriber::builder().finish(),
    )
    .unwrap();

    let bot = Bot::new(&app.bot_token);
    let task_state = std::sync::Arc::new(std::sync::Mutex::new(SharedTaskState {
        state: TaskState::None,
        next_trigger: compute_next_trigger(),
    }));
    let app_state = dptree::deps![std::sync::Arc::clone(&task_state)];
    let scheduled_task = tokio::spawn(send_scheduled_messages(app, task_state, bot.clone()));

    let message_handler = Update::filter_message().endpoint(answer_handler::handle_message);
    let callback_handler =
        Update::filter_callback_query().endpoint(answer_handler::handle_callback_query);
    let handler = dptree::entry()
        .branch(message_handler)
        .branch(callback_handler);

    Dispatcher::builder(bot, handler)
        .dependencies(app_state)
        .build()
        .dispatch()
        .await;
    if let Err(e) = scheduled_task.await {
        tracing::error!("Scheduled task failed: {}", e);
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

fn compute_next_trigger() -> chrono::DateTime<chrono::Local> {
    next_trigger_after(chrono::Local::now())
}

/// Returns the next scheduled trigger relative to `now`.
///
/// Triggers happen at 16:00 (send the order) and 19:00 (control the
/// accomplishment). Before 16:00 the next trigger is today at 16:00, between
/// 16:00 and 19:00 it is today at 19:00, and from 19:00 onwards it rolls over
/// to 16:00 the next day.
fn next_trigger_after(
    now: chrono::DateTime<chrono::Local>,
) -> chrono::DateTime<chrono::Local> {
    if now.hour() < 16 {
        at_hour(now, 16)
    } else if now.hour() < 19 {
        at_hour(now, 19)
    } else {
        at_hour(now + chrono::Duration::days(1), 16)
    }
}

/// Returns `dt` at the given whole hour, with minutes, seconds and
/// sub-seconds zeroed so the trigger lands exactly on the hour.
fn at_hour(
    dt: chrono::DateTime<chrono::Local>,
    hour: u32,
) -> chrono::DateTime<chrono::Local> {
    dt.date_naive()
        .and_hms_opt(hour, 0, 0)
        .and_then(|naive| dt.timezone().from_local_datetime(&naive).single())
        .expect("16:00 and 19:00 are always valid, unambiguous local times")
}

const MAX_COLLECT_ATTEMPTS: u32 = 5;
const INITIAL_BACKOFF_SECS: u64 = 60;
const MAX_BACKOFF_SECS: u64 = 900;

async fn collect_trashes_data_with_retries(
    config: &Config,
) -> Result<data_grabber::TrashesSchedule, error::GstaldergeistError> {
    let mut backoff = INITIAL_BACKOFF_SECS;
    for attempt in 1..=MAX_COLLECT_ATTEMPTS {
        match collect_trashes_data(config).await {
            Ok(schedule) => return Ok(schedule),
            Err(e) => {
                if attempt == MAX_COLLECT_ATTEMPTS {
                    tracing::error!(
                        "Failed to collect trashes data after {} attempts: {}",
                        MAX_COLLECT_ATTEMPTS,
                        e
                    );
                    return Err(e);
                }
                tracing::warn!(
                    "Failed to collect trashes data (attempt {}/{}): {}; retrying in {}s",
                    attempt,
                    MAX_COLLECT_ATTEMPTS,
                    e,
                    backoff
                );
                tokio::time::sleep(tokio::time::Duration::from_secs(backoff)).await;
                backoff = (backoff * 2).min(MAX_BACKOFF_SECS);
            }
        }
    }
    unreachable!("the final attempt returns from the loop");
}

async fn collect_trashes_data(
    config: &Config,
) -> Result<data_grabber::TrashesSchedule, error::GstaldergeistError> {
    let now = chrono::Local::now();
    let today = now.date_naive();
    let weekly = today.weekday() == chrono::Weekday::Sun;
    let until_date = if weekly {
        today + chrono::Duration::days(7)
    } else {
        today + chrono::Duration::days(1)
    };
    let trashes_schedule = data_grabber::get_trashes(&config, today, until_date).await?;
    database::set_trashes(&trashes_schedule.dates)?;
    Ok(trashes_schedule)
}

// Function to send messages on a schedule
async fn send_scheduled_messages(
    config: Config,
    shared_task: std::sync::Arc<std::sync::Mutex<SharedTaskState>>,
    bot: Bot,
) -> Result<(), error::GstaldergeistError> {
    if let Err(e) = collect_trashes_data(&config).await {
        tracing::error!("Initial trash data collection failed: {}", e);
    }
    loop {
        let now = chrono::Local::now();
        let mut next_trigger = shared_task.lock().unwrap().next_trigger;
        if now < next_trigger {
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            continue;
        }
        // A failed collection must not kill the scheduler, or all future
        // reminders silently stop while the bot keeps running.
        let trashes_schedule = match collect_trashes_data_with_retries(&config).await {
            Ok(schedule) => schedule,
            Err(e) => {
                telegram_writer::notify_group(
                    &bot,
                    &config,
                    &format!(
                        "⚠️ I couldn't fetch the trash schedule after {} attempts ({}). \
                         Please check the bins yourselves today.",
                        MAX_COLLECT_ATTEMPTS, e
                    ),
                )
                .await;
                next_trigger = compute_next_trigger();
                shared_task.lock().unwrap().next_trigger = next_trigger;
                continue;
            }
        };
        if next_trigger.hour() >= 19 {
            control_human_accomplishment(&config, shared_task.clone(), &bot, &trashes_schedule)
                .await;
        } else {
            send_order_to_human(&config, shared_task.clone(), &bot, &trashes_schedule).await;
        }
        next_trigger = compute_next_trigger();
        shared_task.lock().unwrap().next_trigger = next_trigger;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn local(year: i32, month: u32, day: u32, hour: u32, min: u32, sec: u32) -> chrono::DateTime<chrono::Local> {
        chrono::Local
            .with_ymd_and_hms(year, month, day, hour, min, sec)
            .single()
            .expect("unambiguous local time")
    }

    #[test]
    fn before_first_trigger_schedules_today_at_16() {
        let now = local(2026, 6, 7, 9, 30, 12);
        assert_eq!(next_trigger_after(now), local(2026, 6, 7, 16, 0, 0));
    }

    #[test]
    fn between_triggers_schedules_today_at_19() {
        let now = local(2026, 6, 7, 16, 30, 45);
        assert_eq!(next_trigger_after(now), local(2026, 6, 7, 19, 0, 0));
    }

    #[test]
    fn after_last_trigger_rolls_over_to_next_day_at_16() {
        let now = local(2026, 6, 7, 19, 5, 0);
        assert_eq!(next_trigger_after(now), local(2026, 6, 8, 16, 0, 0));
    }

    #[test]
    fn at_16_exactly_schedules_19_same_day() {
        let now = local(2026, 6, 7, 16, 0, 0);
        assert_eq!(next_trigger_after(now), local(2026, 6, 7, 19, 0, 0));
    }

    #[test]
    fn at_19_exactly_rolls_over_to_next_day() {
        let now = local(2026, 6, 7, 19, 0, 0);
        assert_eq!(next_trigger_after(now), local(2026, 6, 8, 16, 0, 0));
    }

    #[test]
    fn trigger_zeroes_minutes_seconds_and_subseconds() {
        // 15:59:59 must round to 16:00:00, not carry the 59 seconds over.
        let now = local(2026, 6, 7, 15, 59, 59)
            .with_nanosecond(987_654_321)
            .unwrap();
        let next = next_trigger_after(now);
        assert_eq!(next.minute(), 0);
        assert_eq!(next.second(), 0);
        assert_eq!(next.nanosecond(), 0);
        assert_eq!(next, local(2026, 6, 7, 16, 0, 0));
    }

    #[test]
    fn rollover_keeps_month_and_year_boundaries() {
        let now = local(2026, 12, 31, 20, 0, 0);
        assert_eq!(next_trigger_after(now), local(2027, 1, 1, 16, 0, 0));
    }
}
