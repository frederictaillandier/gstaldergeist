use crate::SharedTaskState;
use crate::TaskState;
use crate::email;
use chrono::{Datelike, TimeZone};
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

/// This function is called when the "Done" button is pressed
async fn done_handler(
    bot: &Bot,
    query: &CallbackQuery,
    task_state: &std::sync::Arc<std::sync::Mutex<SharedTaskState>>,
) -> ResponseResult<()> {
    let Some(message) = query.message.as_ref() else {
        return Ok(());
    };

    let chat_id = message.chat().id;
    let message_id = message.id();
    let _ = bot
        .edit_message_text(chat_id, message_id, "Thank you! Have a nice evening. <3")
        .await?;
    let mut task_state = task_state.lock().unwrap();
    if task_state.state == TaskState::Pending {
        task_state.state = TaskState::None;
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
    return Ok(());
}

async fn cant_handler(
    bot: &Bot,
    query: &CallbackQuery,
    task_state: &std::sync::Arc<std::sync::Mutex<SharedTaskState>>,
) -> ResponseResult<()> {
    let Some(message) = query.message.as_ref() else {
        return Ok(());
    };
    let chat_id = message.chat().id;
    let _ = bot
        .edit_message_text(
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
    return Ok(());
}

async fn request_bags_handler(
    bot: &Bot,
    query: &CallbackQuery,
    _: &std::sync::Arc<std::sync::Mutex<SharedTaskState>>,
) -> ResponseResult<()> {
    let Some(message) = query.message.as_ref() else {
        return Ok(());
    };
    let chat_id = message.chat().id;
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
    Ok(())
}

async fn confirm_request_bags_handler(
    bot: &Bot,
    query: &CallbackQuery,
    _: &std::sync::Arc<std::sync::Mutex<SharedTaskState>>,
) -> ResponseResult<()> {
    let Some(message) = query.message.as_ref() else {
        return Ok(());
    };
    let chat_id = message.chat().id;

    bot.edit_message_text(
        chat_id,
        message.id(),
        "Thank you! I sent a request to We-Recycle.",
    )
    .await?;
    email::request_new_bags();
    Ok(())
}

async fn no_need_bags_handler(
    bot: &Bot,
    query: &CallbackQuery,
    _: &std::sync::Arc<std::sync::Mutex<SharedTaskState>>,
) -> ResponseResult<()> {
    let Some(message) = query.message.as_ref() else {
        return Ok(());
    };
    let chat_id = message.chat().id;

    // Handle "Enough bags" button
    bot.edit_message_text(chat_id, message.id(), "Great! Have a nice evening.")
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
        // Get the chat ID from the message
        let chat_id = message.chat().id;
        match data.as_str() {
            "done" => {
                done_handler(&bot, &query, &task_state).await?;
            }
            "cant" => {
                cant_handler(&bot, &query, &task_state).await?;
            }
            "new_bags" => {
                request_bags_handler(&bot, &query, &task_state).await?;
            }
            "sure_bags" => {
                confirm_request_bags_handler(&bot, &query, &task_state).await?;
            }
            "enough_bags" => {
                no_need_bags_handler(&bot, &query, &task_state).await?;
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

pub async fn handle_message(bot: Bot, msg: Message) -> ResponseResult<()> {
    tracing::info!("Received message: {:?} from {:?}", msg.text(), msg.chat.id);
    if let Some(text) = msg.text() {
        if text == "ping" {
            let trashes = crate::database::get_all_trashes().unwrap();
            let chat_id = msg.chat.id;
            bot.send_message(chat_id, "pong!")
            .await?;

            for (date, waste_types) in trashes {
                bot.send_message(chat_id, format!("{}: {:?}", date, waste_types))
                    .await?;
            }
        }
    }
    Ok(())
}
