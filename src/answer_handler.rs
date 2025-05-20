use teloxide::prelude::*;
use crate::TaskState;
use crate::SharedTaskState;
use chrono::{Datelike, TimeZone};
use teloxide::
    types::{InlineKeyboardButton, InlineKeyboardMarkup};
use crate::email;


// Add this new handler function for callback queries
pub async fn handle_callback_query(
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
