use serenity::all::{
    CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter, Message, Reaction, ReactionType, Timestamp,
};
use serenity::prelude::Context;
use sqlx::{MySqlPool, Row};

use crate::db::get_row_u64;

pub const PIN_ICON_BASE_URL: &str =
    "https://raw.githubusercontent.com/twitter/twemoji/refs/heads/master/assets/72x72/";
pub const PIN_ICON_EXT: &str = ".png";
pub const PIN_ICON_CODES: [&str; 4] = [
    "2b06",  // ⬆️
    "1f53c", // 🔼
    "23eb",  // ⏫
    "1f4c8", // 📈
];

pub fn get_pin_icon_url(index: usize) -> String {
    let code = PIN_ICON_CODES[index];
    format!("{}{}{}", PIN_ICON_BASE_URL, code, PIN_ICON_EXT)
}

pub async fn update_upboard_count(
    pool: &MySqlPool,
    ctx: &Context,
    ret: &Reaction,
    increase: bool,
) -> Result<(Option<Message>, u16), sqlx::Error> {
    match &ret.emoji {
        ReactionType::Unicode(s) if s == "⬆️" => {}
        _ => {
            return Ok((None, 0));
        }
    }

    let msg_id = ret.message_id.get();

    let row = sqlx::query("SELECT count FROM up_board WHERE message_id = ?")
        .bind(msg_id)
        .fetch_optional(pool)
        .await?;

    if let Some(row) = row {
        let current_count: u16 = row
            .try_get::<i32, _>("count")
            .map(|c| c as u16)
            .or_else(|_| row.try_get::<u16, _>("count"))
            .or_else(|_| row.try_get::<u32, _>("count").map(|c| c as u16))
            .unwrap_or(0);

        let new_count = if increase {
            current_count.saturating_add(1)
        } else {
            current_count.saturating_sub(1)
        };

        sqlx::query("UPDATE up_board SET count = ? WHERE message_id = ?")
            .bind(new_count)
            .bind(msg_id)
            .execute(pool)
            .await?;

        Ok((None, new_count))
    } else {
        match ret.channel_id.message(&ctx.http, msg_id).await {
            Ok(msg) => {
                let mut count = 0u16;

                for reaction in &msg.reactions {
                    if let ReactionType::Unicode(s) = &reaction.reaction_type {
                        if s == "⬆️" {
                            count = count.saturating_add(reaction.count as u16);
                        }
                    }
                }

                sqlx::query("INSERT INTO up_board (pin_id, message_id, count) VALUES (0, ?, ?)")
                    .bind(msg_id)
                    .bind(count)
                    .execute(pool)
                    .await?;

                Ok((Some(msg), count))
            }
            Err(e) => {
                eprintln!("[ERROR] Failed to fetch message for upboard: {:?}", e);
                Err(sqlx::Error::RowNotFound)
            }
        }
    }
}

pub async fn update_upboard_pin(
    pool: &MySqlPool,
    message_id: u64,
    pin_id: u64,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE up_board SET pin_id = ? WHERE message_id = ?")
        .bind(pin_id)
        .bind(message_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_upboard_pin(pool: &MySqlPool, message_id: u64) -> Result<u64, sqlx::Error> {
    let row = sqlx::query("SELECT pin_id FROM up_board WHERE message_id = ?")
        .bind(message_id)
        .fetch_optional(pool)
        .await?;

    Ok(row.and_then(|r| get_row_u64(&r, "pin_id")).unwrap_or(0))
}

#[allow(dead_code)]
pub async fn exists_upboard_pin_id(pool: &MySqlPool, pin_id: u64) -> Result<bool, sqlx::Error> {
    let row = sqlx::query("SELECT 1 FROM up_board WHERE pin_id = ? LIMIT 1")
        .bind(pin_id)
        .fetch_optional(pool)
        .await?;

    Ok(row.is_some())
}

pub fn build_pin_embed(message: &Message, count: u16) -> CreateEmbed {
    let mut embed = CreateEmbed::new()
        .description(&message.content)
        .url(message.link())
        .timestamp(Timestamp::now())
        .footer(
            CreateEmbedFooter::new(&message.author.name).icon_url(
                message
                    .author
                    .avatar_url()
                    .unwrap_or_else(|| message.author.default_avatar_url()),
            ),
        );

    let mut image_inserted = false;
    let mut more_image = false;
    for attachment in &message.attachments {
        if let Some(content_type) = &attachment.content_type {
            if content_type.starts_with("image/") {
                if !image_inserted {
                    embed = embed.image(&attachment.url);
                    image_inserted = true;
                } else {
                    more_image = true;
                    break;
                }
            }
        }
    }

    if more_image {
        embed = embed.title("PIN_IMAGE+");
    } else {
        embed = embed.title("PIN");
    }

    if count >= 10 {
        embed = embed
            .author(CreateEmbedAuthor::new(count.to_string()).icon_url(get_pin_icon_url(3)))
            .color(0xFA05C1);
    } else if count >= 8 {
        embed = embed
            .author(CreateEmbedAuthor::new(count.to_string()).icon_url(get_pin_icon_url(2)))
            .color(0xA805FA);
    } else if count >= 5 {
        embed = embed
            .author(CreateEmbedAuthor::new(count.to_string()).icon_url(get_pin_icon_url(1)))
            .color(0x1105FA);
    } else {
        embed = embed
            .author(CreateEmbedAuthor::new(count.to_string()).icon_url(get_pin_icon_url(0)))
            .color(0x05B5FA);
    }

    embed
}
