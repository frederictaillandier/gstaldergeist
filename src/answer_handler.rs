use crate::SharedTaskState;
use crate::TaskState;
use crate::email;
use chrono::TimeZone;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, MessageId};

/// This function is called when the "Done" button is pressed
async fn done_handler(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    task_state: &std::sync::Arc<std::sync::Mutex<SharedTaskState>>,
) -> ResponseResult<()> {
    let _ = bot
        .edit_message_text(chat_id, message_id, "Thank you! Have a nice evening. <3")
        .await?;
    let mut task_state = task_state.lock().unwrap();
    if task_state.state == TaskState::Pending {
        task_state.state = TaskState::None;
        task_state.next_trigger = next_evening_trigger(chrono::Local::now());
    }
    return Ok(());
}

/// 18:00 local on the day after `now`. The hour is fixed and never lands on a
/// DST transition, so the resulting local time is always unambiguous.
fn next_evening_trigger(
    now: chrono::DateTime<chrono::Local>,
) -> chrono::DateTime<chrono::Local> {
    let tomorrow = now.date_naive() + chrono::Duration::days(1);
    tomorrow
        .and_hms_opt(18, 0, 0)
        .and_then(|naive| now.timezone().from_local_datetime(&naive).single())
        .expect("18:00 is always a valid, unambiguous local time")
}

async fn cant_handler(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    task_state: &std::sync::Arc<std::sync::Mutex<SharedTaskState>>,
) -> ResponseResult<()> {
    let _ = bot
        .edit_message_text(
            chat_id,
            message_id,
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
    return Ok(());
}

async fn request_bags_handler(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
) -> ResponseResult<()> {
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
        message_id,
        "Are you sure ? A request will be sent to We-Recycle.",
    )
    .reply_markup(keyboard)
    .await?;
    Ok(())
}

async fn confirm_request_bags_handler(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
) -> ResponseResult<()> {
    let reply = match email::request_new_bags() {
        Ok(()) => "Thank you! I sent a request to We-Recycle.",
        Err(e) => {
            tracing::error!("Failed to send bag request email: {}", e);
            "Sorry, I couldn't send the request to We-Recycle. Please try again later."
        }
    };

    bot.edit_message_text(chat_id, message_id, reply).await?;
    Ok(())
}

async fn no_need_bags_handler(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
) -> ResponseResult<()> {
    // Handle "Enough bags" button
    bot.edit_message_text(chat_id, message_id, "Great! Have a nice evening.")
        .await?;
    Ok(())
}

// Add this new handler function for callback queries
pub async fn handle_callback_query(
    bot: Bot,
    query: CallbackQuery,
    task_state: std::sync::Arc<std::sync::Mutex<SharedTaskState>>,
) -> ResponseResult<()> {
    let data = query.data.clone();
    let Some(message) = query.message.clone() else {
        return Ok(());
    };

    // Extract the callback data from the query
    if let Some(data) = &data {
        // Get the chat and message IDs once; every handler edits this message.
        let chat_id = message.chat().id;
        let message_id = message.id();
        match data.as_str() {
            "done" => {
                done_handler(&bot, chat_id, message_id, &task_state).await?;
            }
            "cant" => {
                cant_handler(&bot, chat_id, message_id, &task_state).await?;
            }
            "new_bags" => {
                request_bags_handler(&bot, chat_id, message_id).await?;
            }
            "sure_bags" => {
                confirm_request_bags_handler(&bot, chat_id, message_id).await?;
            }
            "enough_bags" => {
                no_need_bags_handler(&bot, chat_id, message_id).await?;
            }
            _ => {
                bot.send_message(chat_id, "Unrecognized option.").await?;
            }
        }
        // Answer the callback query to remove the "loading" state
        bot.answer_callback_query(query.id).await?;
    }

    Ok(())
}

pub async fn handle_message(
    bot: Bot,
    msg: Message,
    task_state: std::sync::Arc<std::sync::Mutex<SharedTaskState>>,
) -> ResponseResult<()> {
    tracing::info!("Received message: {:?} from {:?}", msg.text(), msg.chat.id);
    if let Some(text) = msg.text() {
        if text == "ping" {
            let chat_id = msg.chat.id;
            bot.send_message(chat_id, "pong!").await?;

            let time = chrono::Local::now();
            bot.send_message(
                chat_id,
                format!(
                    "now is {}, next trigger is {}",
                    time,
                    task_state.lock().unwrap().next_trigger
                ),
            )
            .await?;
            let trashes = crate::database::get_all_trashes();
            tracing::info!("Trashes: {:?}", trashes);

            match trashes {
                Ok(trashes) => {
                    for (date, waste_types) in trashes {
                        bot.send_message(chat_id, format!("{}: {:?}", date, waste_types))
                            .await?;
                    }
                }
                Err(e) => {
                    tracing::error!("Error getting trashes: {:?}", e);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    fn local(year: i32, month: u32, day: u32, hour: u32, min: u32, sec: u32) -> chrono::DateTime<chrono::Local> {
        chrono::Local
            .with_ymd_and_hms(year, month, day, hour, min, sec)
            .single()
            .expect("unambiguous local time")
    }

    #[test]
    fn triggers_at_18_the_following_day() {
        let now = local(2026, 6, 13, 16, 30, 0);
        assert_eq!(next_evening_trigger(now), local(2026, 6, 14, 18, 0, 0));
    }

    #[test]
    fn rolls_over_month_and_year_boundaries() {
        assert_eq!(
            next_evening_trigger(local(2026, 6, 30, 20, 0, 0)),
            local(2026, 7, 1, 18, 0, 0)
        );
        assert_eq!(
            next_evening_trigger(local(2026, 12, 31, 23, 59, 59)),
            local(2027, 1, 1, 18, 0, 0)
        );
    }

    #[test]
    fn zeroes_minutes_seconds_and_subseconds() {
        let now = local(2026, 6, 13, 16, 45, 31)
            .with_nanosecond(123_456_789)
            .unwrap();
        let next = next_evening_trigger(now);
        assert_eq!((next.hour(), next.minute(), next.second()), (18, 0, 0));
        assert_eq!(next.nanosecond(), 0);
    }
}
