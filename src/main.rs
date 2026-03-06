use serenity::async_trait;
use serenity::builder::{CreateInteractionResponse, CreateInteractionResponseMessage, CreateInteractionResponseFollowup};
use serenity::model::channel::{Message};
use serenity::model::gateway::{Ready};
use serenity::model::application::{Interaction};
use serenity::model::id::{GuildId, ChannelId, UserId, MessageId};
use serenity::model::prelude::{OnlineStatus, Command};
use serenity::prelude::*;
use serenity::prelude::Context;
use std::env;
use std::collections::HashMap;
use dotenvy::dotenv;
use serenity::all::{
    ActionRowComponent, ActivityData, ButtonStyle, ChannelType, Color,
    CommandOptionType, CommandType, CreateActionRow, CreateAllowedMentions, CreateAttachment,
    CreateButton, CreateChannel, CreateCommand, CreateCommandOption, CreateEmbed, CreateEmbedFooter,
    CreateMessage, EditAttachments, EditInteractionResponse, EditMember, EditMessage, EditRole,
    GuildMemberUpdateEvent, InstallationContext, Member, MessageReference, MessageReferenceKind,
    MessageUpdateEvent, Permissions, Reaction, ReactionType, Request, RoleId, Route, StickerType,
    Timestamp, TypingStartEvent, CreateEmbedAuthor, GetMessages, Channel
};
use chrono::{Utc, Local, Duration, Timelike, Datelike, DateTime};
use once_cell::sync::Lazy;
use sqlx::{MySql, Pool, Row};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::time::sleep;
use tokio::sync::Notify;
use tokio::sync::OnceCell;
use image::{Pixel, RgbImage, ImageBuffer, Rgb};
use serenity::json::Value;
use serenity::http::LightMethod;
use sqlx::MySqlPool;

mod response;
use response::{
    get_response, get_special_response,
    Resp, RespStore, db_add_key, db_add_key_bulk, db_add_resp, db_add_resp_bulk, db_get_table, db_remove_table, db_remove_resp, db_new_key
};

static TOKEN: once_cell::sync::OnceCell<String> = once_cell::sync::OnceCell::new();
static DATABASE_URL: once_cell::sync::OnceCell<String> = once_cell::sync::OnceCell::new();
static DATABASE_URL_TOKIDM: once_cell::sync::OnceCell<String> = once_cell::sync::OnceCell::new();

static BOT_USERID: UserId = UserId::new(1286264098987835445);
static GUILD_ID: GuildId = GuildId::new(1027978441573793853);

static KKITUT_USERID: UserId = UserId::new(700176751422341201);

static CHANNELID_LOG_SECRET: Lazy<ChannelId> = Lazy::new(|| ChannelId::new(1208334066873798726));
static CHANNELID_PIN: Lazy<ChannelId> = Lazy::new(|| ChannelId::new(1399760051291426847));

macro_rules! sr {
    ($a:expr) => {
        if let Some(r) = $a {
            r
        } else {
            return;
        }
    };
}

macro_rules! er {
    ($res:expr, $msg:expr $(,)?) => {{
        match $res {
            Ok(v) => v,
            Err(e) => {
                eprintln!("{}: {}", $msg, e);
                return;
            }
        }
    }};
}

struct Handler {
    is_ready: Arc<AtomicBool>,
    delete_messages: Arc<Mutex<Vec<(u64, (u64, u64, u64), (u64, u64, u64), chrono::DateTime<Local>)>>>, //user, first, end, time. tuple(id, guild, channel)
    stop_flag: Arc<AtomicBool>,
    stop_notify: Arc<Notify>,
    resp: Arc<Mutex<Vec<Resp>>>,
    resp_store: Arc<RespStore>,
    blacklist_guilds: Arc<Mutex<Vec<u64>>>,
    dm_channels: Arc<Mutex<Vec<(u64, u64)>>>,
    trap_ids: Arc<Mutex<HashMap<u64, (u64, Option<u64>)>>>,
}

fn lng_trs<T>(is_korean: bool, en: T, ko: T) -> T {
    if is_korean {
        ko
    } else {
        en
    }
}

static POOL: OnceCell<MySqlPool> = OnceCell::const_new();
static POOL_TOKIDM: OnceCell<MySqlPool> = OnceCell::const_new();

async fn get_pool() -> Result<&'static MySqlPool, sqlx::Error> {
    POOL
        .get_or_try_init(|| async {
            Pool::<MySql>::connect(DATABASE_URL.get().unwrap()).await
        })
        .await
}

async fn get_pool_tokidm() -> Result<&'static MySqlPool, sqlx::Error> {
    POOL_TOKIDM
        .get_or_try_init(|| async {
            Pool::<MySql>::connect(DATABASE_URL_TOKIDM.get().unwrap()).await
        })
        .await
}

fn update_avatar_size(url: &str) -> String {
    if url.contains("cdn.discordapp.com/embed/avatars/") {
        return url.to_string();
    }

    if url.contains("?size=") {
        let parts: Vec<&str> = url.split("?size=").collect();
        parts[0].to_string() + "?size=4096"
    } else {
        url.to_string() + "?size=4096"
    }
}

async fn fetch_image_from_url(url: &str) -> Result<ImageBuffer<Rgb<u8>, Vec<u8>>, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let res = client.get(url).send().await?.bytes().await?;
    let img = image::load_from_memory(&res)?.to_rgb8();
    Ok(img)
}

fn average_center_color(img: &RgbImage) -> image::Rgb<u8> {
    let (w, h) = img.dimensions();
    let side = (w.min(h) as f32 * 0.7).round() as u32;
    let x0 = (w - side) / 2;
    let y0 = (h - side) / 2;

    let mut r_sum = 0u64;
    let mut g_sum = 0u64;
    let mut b_sum = 0u64;
    let mut count = 0u64;

    for y in y0..(y0 + side) {
        for x in x0..(x0 + side) {
            let pixel = img.get_pixel(x, y).to_rgb();
            let [r, g, b] = pixel.0;
            r_sum += r as u64;
            g_sum += g as u64;
            b_sum += b as u64;
            count += 1;
        }
    }

    image::Rgb([
        (r_sum / count) as u8,
        (g_sum / count) as u8,
        (b_sum / count) as u8,
    ])
}

async fn get_user_info(http: &serenity::http::Http, user_id: serenity::model::id::UserId) -> Result<(String, String), Box<dyn std::error::Error>> {
    let route = Route::User { user_id };
    let request = Request::new(route, LightMethod::Get);

    let response = http.request(request).await?;
    let bytes = response.bytes().await?;
    let json: serde_json::Value = serde_json::from_slice(&bytes)?;

    let username = json["username"].as_str().unwrap_or("Unknown").to_string();

    let avatar_url = if let Some(avatar_hash) = json["avatar"].as_str() {
        format!("https://cdn.discordapp.com/avatars/{}/{}.png", user_id.get(), avatar_hash)
    } else {
        "No avatar".to_string()
    };

    Ok((username, avatar_url))
}

async fn get_vmrss_mb() -> Result<f64, String> {
    use tokio::fs::File;
    use tokio::io::{BufReader, AsyncBufReadExt};

    let file = File::open("/proc/self/status").await
        .map_err(|e| format!("Failed to open /proc/self/status: {:?}", e))?;

    let mut reader = BufReader::new(file);
    let mut line = String::new();

    while reader.read_line(&mut line).await.map_err(|e| e.to_string())? > 0 {
        if line.starts_with("VmRSS:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let kb = parts[1].parse::<f64>()
                    .map_err(|e| format!("Failed to parse VmRSS value: {:?}", e))?;
                return Ok(kb / 1024.0);
            }
        }
        line.clear();
    }
    Err("VmRSS line not found".to_string())
}

const PIN_ICON_BASE_URL: &str = "https://raw.githubusercontent.com/twitter/twemoji/refs/heads/master/assets/72x72/";
const PIN_ICON_EXT: &str = ".png";
const PIN_ICON_CODES: [&str; 4] = [
    "2b06",   // ⬆️
    "1f53c",  // 🔼
    "23eb",   // ⏫
    "1f4c8",  // 📈
];
fn get_pin_icon_url(index: usize) -> String {
    let code = PIN_ICON_CODES[index];
    format!("{}{}{}", PIN_ICON_BASE_URL, code, PIN_ICON_EXT)
}
pub async fn update_upboard_count(
    pool: &MySqlPool,
    ctx: &serenity::prelude::Context,
    ret: &serenity::all::Reaction,
    increase: bool,
) -> Result<(Option<Message>, u16), sqlx::Error> {
    match &ret.emoji {
        ReactionType::Unicode(s) if s == "⬆️" => {}
        _ => {
            return Ok((None, 0));
        }
    }

    let msg_id = ret.message_id.get();

    let row = sqlx::query!(
        "SELECT count FROM up_board WHERE message_id = ?",
        msg_id
    )
    .fetch_optional(pool)
    .await?;

    if let Some(row) = row {
        let current_count = row.count as u16;
        let new_count = if increase {
            current_count.saturating_add(1)
        } else {
            current_count.saturating_sub(1)
        };

        sqlx::query!(
            "UPDATE up_board SET count = ? WHERE message_id = ?",
            new_count,
            msg_id
        )
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

                sqlx::query!(
                    "INSERT INTO up_board (pin_id, message_id, count) VALUES (0, ?, ?)",
                    msg_id,
                    count
                )
                .execute(pool)
                .await?;

                Ok((Some(msg), count))
            }
            Err(e) => {
                eprintln!("[ERROR] Failed to fetch message for upboard: {:?}", e);
                return Err(sqlx::Error::RowNotFound);
            }
        }
    }
}
pub async fn update_upboard_pin(
    pool: &MySqlPool,
    message_id: u64,
    pin_id: u64,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE up_board SET pin_id = ? WHERE message_id = ?",
        pin_id as u64,
        message_id as u64
    )
    .execute(pool)
    .await?;
    Ok(())
}
pub async fn get_upboard_pin(
    pool: &MySqlPool,
    message_id: u64,
) -> Result<u64, sqlx::Error> {
    let row = sqlx::query!(
        "SELECT pin_id FROM up_board WHERE message_id = ?",
        message_id as u64
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.pin_id as u64).unwrap_or(0))
}
pub async fn exists_upboard_pin_id(
    pool: &MySqlPool,
    pin_id: u64,
) -> Result<bool, sqlx::Error> {
    let row = sqlx::query_scalar!(
        "SELECT 1 FROM up_board WHERE pin_id = ? LIMIT 1",
        pin_id as u64
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.is_some())
}
fn build_pin_embed(message: &Message, count: u16) -> CreateEmbed {
    let mut embed = CreateEmbed::new()
        .description(&message.content)
        .url(message.link())
        .timestamp(Timestamp::now())
        .footer(
            CreateEmbedFooter::new(&message.author.name)
                .icon_url(message.author.avatar_url().unwrap_or_else(|| message.author.default_avatar_url())),
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
        embed = embed.author(
            CreateEmbedAuthor::new(count.to_string()).icon_url(get_pin_icon_url(3))
        ).color(0xFA05C1);
    } else if count >= 8 {
        embed = embed.author(
            CreateEmbedAuthor::new(count.to_string()).icon_url(get_pin_icon_url(2))
        ).color(0xA805FA);
    } else if count >= 5 {
        embed = embed.author(
            CreateEmbedAuthor::new(count.to_string()).icon_url(get_pin_icon_url(1))
        ).color(0x1105FA);
    } else {
        embed = embed.author(
            CreateEmbedAuthor::new(count.to_string()).icon_url(get_pin_icon_url(0))
        ).color(0x05B5FA);
    }

    embed
}

fn parse_quoted<'a>(
    iter: &mut impl Iterator<Item = &'a str>,
    arg_name: &str,
    max_len: usize
) -> Result<String, String> {
    let first = iter.next().ok_or_else(|| format!("{} null", arg_name))?;
    if !first.starts_with('"') {
        return Err(format!("{} must start with a quote (\")", arg_name));
    }

    let mut text = first.trim_start_matches('"').trim_end_matches('"').to_string();
    let mut found_end_quote = first.ends_with('"');

    while !found_end_quote {
        if let Some(next) = iter.next() {
            text.push(' ');
            if next.ends_with('"') {
                text.push_str(&next[..next.len() - 1]);
                found_end_quote = true;
            } else {
                text.push_str(next);
            }
        } else {
            return Err(format!("{} missing closing quote (\")", arg_name));
        }
    }

    if text.len() > max_len {
        return Err(format!("{} length must be <= {}", arg_name, max_len));
    }

    Ok(text)
}

#[async_trait]
impl EventHandler for Handler {
    async fn guild_member_update(&self, ctx: Context, old: Option<Member>, _: Option<Member>, event: GuildMemberUpdateEvent) {
        let target_role = RoleId::new(1366815072982401064);
        let mut change_auto_color = false;

        match &old {
            None => {
                if event.roles.contains(&target_role) {
                    change_auto_color = true;
                }
            }
            Some(old_member) => {
                let had_role = old_member.roles.contains(&target_role);
                let has_role = event.roles.contains(&target_role);

                match (had_role, has_role) {
                    (false, true) => {
                        change_auto_color = true;
                    }
                    (true, true) => {
                        if old_member.user.avatar != event.user.avatar {
                            change_auto_color = true;
                        }
                    }
                    (true, false) => {
                        match event.guild_id.roles(&ctx.http).await {
                            Ok(roles_map) => {
                                let member = match event.guild_id.member(&ctx.http, event.user.id).await {
                                    Ok(m) => m,
                                    Err(e) => {
                                        eprintln!("[ERROR] fetch member: {:?}", e);
                                        return;
                                    }
                                };
                                for role_id in &member.roles {
                                    if let Some(role) = roles_map.get(role_id) {
                                        if role.name.starts_with(':') {
                                            if let Err(e) = member.remove_role(&ctx.http, *role_id).await {
                                                eprintln!("[ERROR] remove_role: {:?}", e);
                                                return;
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("[ERROR] fetch roles: {:?}", e);
                                return;
                            }
                        }
                    }
                    (false, false) => {
                        return;
                    }
                }
            }
        }

        if change_auto_color {
            let avatar_url = event.user.avatar_url().unwrap_or_else(|| event.user.default_avatar_url());

            match fetch_image_from_url(&avatar_url).await {
                Ok(img) => {
                    let color = average_center_color(&img);
                    let hex_color = format!("{:02x}{:02x}{:02x}", color.0[0], color.0[1], color.0[2]);

                    match event.guild_id.roles(&ctx.http).await {
                        Ok(roles_map) => {
                            let existing_role = roles_map.values().find(|r| {
                                let name = r.name.to_lowercase();
                                name.len() >= 6 && &name[name.len() - 6..] == hex_color
                            });
                    
                            let member = match event.guild_id.member(&ctx.http, event.user.id).await {
                                Ok(m) => m,
                                Err(e) => {
                                    eprintln!("[ERROR] event.guild_id.member: {:?}", e);
                                    return;
                                }
                            };
                    
                            if existing_role.as_ref().map(|r| member.roles.contains(&r.id)).unwrap_or(false) {
                                return;
                            }
                    
                            for role_id in &member.roles {
                                if let Some(role) = roles_map.get(role_id) {
                                    if role.name.starts_with(':') {
                                        if let Err(e) = member.remove_role(&ctx.http, *role_id).await {
                                            eprintln!("[ERROR] remove_role: {:?}", e);
                                            return;
                                        }
                                    }
                                }
                            }
                    
                            let new_role = if let Some(role) = existing_role {
                                role.clone()
                            } else {
                                let roles_map = match event.guild_id.roles(&ctx.http).await {
                                    Ok(map) => map,
                                    Err(e) => {
                                        eprintln!("[ERROR] roles after creation: {:?}", e);
                                        return;
                                    }
                                };
                    
                                let autoindex_role_id = RoleId::new(1366828051857543228);
                                let mut sorted_roles: Vec<_> = roles_map.iter().collect();
                                sorted_roles.sort_by_key(|(_, r)| r.position);
                    
                                let mut target_position = 0;
                                if let Some((idx, _)) = sorted_roles.iter().enumerate().find(|(_, (&id, _))| id == autoindex_role_id) {
                                    target_position = sorted_roles[idx].1.position + 1;
                                }
                    
                                let [r, g, b] = color.0;
                                let rolebuilder = EditRole::new()
                                    .colour(Color::from_rgb(r, g, b))
                                    .name(format!(":{}", hex_color))
                                    .position(target_position)
                                    .permissions(Permissions::empty());
                    
                                match event.guild_id.create_role(&ctx.http, rolebuilder).await {
                                    Ok(role) => role,
                                    Err(e) => {
                                        eprintln!("[ERROR] create_role: {:?}", e);
                                        return;
                                    }
                                }
                            };
                    
                            if let Err(e) = member.add_role(&ctx.http, new_role.id).await {
                                eprintln!("[ERROR] add_role: {:?}", e);
                                return;
                            }
                        }
                        Err(e) => {
                            eprintln!("[ERROR] event.guild_id.roles: {:?}", e);
                            return;
                        }
                    }                                   
                }
                Err(e) => {
                    eprintln!("[ERROR] fetch_image_from_url: {:?}", e);
                }
            }
        }
    }

    async fn typing_start(&self, ctx: Context, event: TypingStartEvent) {
        if event.user_id == BOT_USERID {
            return;
        }
        let channel_id_u64 = event.channel_id.get();

        if let Some(guild_id) = event.guild_id {
            if guild_id.get() != GUILD_ID.get() {
                return;
            }

            if let Some((_, user_channel_id)) = {
                let guard = self.dm_channels.lock().await;
                guard.iter()
                    .find(|&&(bot_channel_id, _)| bot_channel_id == channel_id_u64)
                    .copied()
            } {
                if let Err(e) = ChannelId::new(user_channel_id)
                    .broadcast_typing(&ctx.http)
                    .await {
                    eprintln!("[ERROR] send typing indicator: {:?}", e);
                }
            }
        } else {
            if let Some((bot_channel_id, _)) = {
                let guard = self.dm_channels.lock().await;
                guard.iter()
                    .find(|&&(_, user_channel_id)| user_channel_id == channel_id_u64)
                    .copied()
            } {
                if let Err(e) = ChannelId::new(bot_channel_id)
                    .broadcast_typing(&ctx.http)
                    .await {
                    eprintln!("[ERROR] send typing indicator: {:?}", e);
                }
            }
        } 
    }

    async fn reaction_add(&self, ctx: Context, ret: Reaction) {
        let user_id = sr!(ret.user_id);

        if user_id == BOT_USERID {
            return;
        }
        if let Some(guild_id) = ret.guild_id {
            if guild_id.get() != GUILD_ID.get() {
                return;
            }
            
            let channel_id_u64 = ret.channel_id.get();

            let dm_pair = {
                let guard = self.dm_channels.lock().await;
                guard.iter()
                    .find(|&&(bot_ch, _)| bot_ch == channel_id_u64)
                    .copied()
            };

            if let Some((bot_channel_id, user_channel_id)) = dm_pair {
                let pool = er!(get_pool_tokidm().await, "[ERROR] DM reaction_add bot->user get_pool_tokidm()");

                let table_name = format!("c{}u{}", bot_channel_id, user_channel_id);

                let query = format!(
                    "SELECT bot, user FROM {} WHERE user = ?",
                    table_name
                );

                let mut result = sqlx::query(&query)
                    .bind(ret.message_id.get())
                    .fetch_optional(pool)
                    .await
                    .unwrap();

                if result.is_none() {
                    let query = format!(
                        "SELECT bot, user FROM {} WHERE bot = ?",
                        table_name
                    );
                    result = sqlx::query(&query)
                        .bind(ret.message_id.get())
                        .fetch_optional(pool)
                        .await
                        .unwrap();
                }

                if let Some(row) = result {
                    let user_message_id: u64 = row.get("user");
                    let bot_message_id: u64 = row.get("bot");

                    let target_message_id =
                        if ret.message_id.get() == user_message_id {
                            bot_message_id
                        } else {
                            user_message_id
                        };

                    if let Err(e) = ChannelId::new(user_channel_id)
                        .create_reaction(&ctx.http, target_message_id, ret.emoji)
                        .await
                    {
                        eprintln!(
                            "[ERROR] create_reaction in DM {}: {:?}",
                            user_channel_id, e
                        );
                    }
                }
            } else {
                let pool = er!(get_pool().await, "[ERROR] Upboard reaction_add get_pool()");

                match update_upboard_count(&pool, &ctx, &ret, true).await {
                    Ok((message, count)) => {
                        let lets_up_board = count >= 3;

                        let message = match message {
                            Some(m) => m,
                            None => match ret.message(&ctx.http).await {
                                Ok(m) => m,
                                Err(e) => {
                                    eprintln!("[ERROR] fetch message: {:?}", e);
                                    return;
                                }
                            }
                        };

                        if lets_up_board {
                            match get_upboard_pin(&pool, message.id.get()).await {
                                Ok(pin_id) => {
                                    let embed = build_pin_embed(&message, count);

                                    if pin_id == 0 {
                                        let builder = CreateMessage::new().embed(embed);
                                        if let Ok(pinmsg) =
                                            CHANNELID_PIN.send_message(&ctx.http, builder).await
                                        {
                                            let _ = update_upboard_pin(
                                                &pool,
                                                ret.message_id.get(),
                                                pinmsg.id.get()
                                            ).await;
                                        }
                                    } else {
                                        let builder = EditMessage::new().embed(embed);
                                        let _ = CHANNELID_PIN
                                            .edit_message(&ctx.http, MessageId::new(pin_id), builder)
                                            .await;
                                    }
                                }
                                Err(e) => {
                                    eprintln!("[ERROR] get_upboard_pin(): {:?}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[ERROR] update_upboard_count: {:?}", e);
                    }
                }
            }
        } else {
            let channel_id_u64 = ret.channel_id.get();

            let dm_pair = {
                let guard: tokio::sync::MutexGuard<'_, Vec<(u64, u64)>> = self.dm_channels.lock().await;
                guard.iter()
                    .find(|&&(_, user_ch)| user_ch == channel_id_u64)
                    .copied()
            };

            if let Some((bot_channel_id, user_channel_id)) = dm_pair {
                let pool = er!(get_pool_tokidm().await, "[ERROR] DM reaction_add user->bot get_pool_tokidm()");
                let table_name = format!("c{}u{}", bot_channel_id, user_channel_id);

                let query = format!(
                    "SELECT bot, user FROM {} WHERE user = ?",
                    table_name
                );

                let mut result = sqlx::query(&query)
                    .bind(ret.message_id.get())
                    .fetch_optional(pool)
                    .await
                    .unwrap();

                if result.is_none() {
                    let query = format!(
                        "SELECT bot, user FROM {} WHERE bot = ?",
                        table_name
                    );
                    result = sqlx::query(&query)
                        .bind(ret.message_id.get())
                        .fetch_optional(pool)
                        .await
                        .unwrap();
                }

                if let Some(row) = result {
                    let user_message_id: u64 = row.get("user");
                    let bot_message_id: u64 = row.get("bot");

                    let target_message_id =
                        if ret.message_id.get() == user_message_id {
                            bot_message_id
                        } else {
                            user_message_id
                        };

                    if let Err(e) = ChannelId::new(bot_channel_id)
                        .create_reaction(&ctx.http, target_message_id, ret.emoji)
                        .await
                    {
                        eprintln!(
                            "[ERROR] create_reaction in Channel {}: {:?}",
                            bot_channel_id, e
                        );
                    }
                }
            } 
        } 
    }
    async fn reaction_remove(&self, ctx: Context, ret: Reaction) {
        let user_id = sr!(ret.user_id);

        if user_id == BOT_USERID {
            return;
        }

        let channel_id_u64 = ret.channel_id.get();

        let dm_pair = {
            let guard = self.dm_channels.lock().await;
            guard.iter()
                .find(|&&(bot_ch, user_ch)| {
                    bot_ch == channel_id_u64 || user_ch == channel_id_u64
                })
                .copied()
        };

        if let Some((bot_channel_id, user_channel_id)) = dm_pair {
            let pool = er!(get_pool_tokidm().await, "[ERROR] DM reaction_remove get_pool_tokidm()");
            let table_name = format!("c{}u{}", bot_channel_id, user_channel_id);

            let query = format!(
                "SELECT bot, user FROM {} WHERE user = ?",
                table_name
            );

            let mut result = sqlx::query(&query)
                .bind(ret.message_id.get())
                .fetch_optional(pool)
                .await
                .unwrap();

            if result.is_none() {
                let query = format!(
                    "SELECT bot, user FROM {} WHERE bot = ?",
                    table_name
                );
                result = sqlx::query(&query)
                    .bind(ret.message_id.get())
                    .fetch_optional(pool)
                    .await
                    .unwrap();
            }

            if let Some(row) = result {
                let user_message_id: u64 = row.get("user");
                let bot_message_id: u64 = row.get("bot");

                let target_message_id =
                    if ret.message_id.get() == user_message_id {
                        bot_message_id
                    } else {
                        user_message_id
                    };

                let target_channel_id =
                    if channel_id_u64 == bot_channel_id {
                        user_channel_id
                    } else {
                        bot_channel_id
                    };

                if let Err(e) = ChannelId::new(target_channel_id)
                    .delete_reaction(
                        &ctx.http,
                        MessageId::new(target_message_id),
                        Some(BOT_USERID),
                        ret.emoji
                    )
                    .await
                {
                    eprintln!(
                        "[ERROR] remove reaction in Channel {}: {:?}",
                        target_channel_id, e
                    );
                }
            }

            return;
        }

        if let Some(guild_id) = ret.guild_id {
            if guild_id.get() != GUILD_ID.get() {
                return;
            }

            let pool = er!(get_pool().await, "[ERROR] Upboard reaction_remove get_pool()");

            match update_upboard_count(&pool, &ctx, &ret, false).await {
                Ok((_, count)) => {
                    let lets_delete_board = count < 3;

                    match get_upboard_pin(&pool, ret.message_id.get()).await {
                        Ok(pin_id) => {
                            if pin_id != 0 {
                                if lets_delete_board {
                                    if let Err(e) =
                                        CHANNELID_PIN
                                            .delete_message(&ctx.http, MessageId::new(pin_id))
                                            .await
                                    {
                                        eprintln!(
                                            "[ERROR] Failed to delete pin message: {}",
                                            e
                                        );
                                    } else {
                                        let _ = update_upboard_pin(
                                            &pool,
                                            ret.message_id.get(),
                                            0
                                        ).await;
                                    }
                                } else {
                                    match ret.message(&ctx.http).await {
                                        Ok(original_message) => {
                                            let embed =
                                                build_pin_embed(&original_message, count);
                                            let builder =
                                                EditMessage::new().embed(embed);
                                            let _ = CHANNELID_PIN
                                                .edit_message(
                                                    &ctx.http,
                                                    MessageId::new(pin_id),
                                                    builder
                                                )
                                                .await;
                                        }
                                        Err(e) => {
                                            eprintln!(
                                                "[ERROR] get pin original message(): {:?}",
                                                e
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("[ERROR] get_upboard_pin(): {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[ERROR] update_upboard_count: {:?}", e);
                }
            }
        }
    }

    async fn message_delete(&self, ctx: Context, channel_id: ChannelId, message_id: MessageId, guild_id: Option<GuildId>) {
        let channel_id_u64 = channel_id.get();

        let dm_pair = {
            let guard = self.dm_channels.lock().await;
            guard.iter()
                .find(|&&(bot_ch, user_ch)| {
                    bot_ch == channel_id_u64 || user_ch == channel_id_u64
                })
                .copied()
        };

        if let Some((bot_channel_id, user_channel_id)) = dm_pair {
            let pool = er!(get_pool_tokidm().await, "[ERROR] DM message_delete get_pool_tokidm()");
            let table_name = format!("c{}u{}", bot_channel_id, user_channel_id);

            if let Some(guild_id) = guild_id {
                if guild_id.get() != GUILD_ID.get() {
                    return;
                }

                let query = format!(
                    "SELECT bot FROM {} WHERE user = ?",
                    table_name
                );

                if let Ok(row) = sqlx::query(&query)
                    .bind(message_id.get())
                    .fetch_one(pool)
                    .await
                {
                    use sqlx::Row;

                    let bot_message_id: u64 = row.get("bot");

                    if let Err(e) = ChannelId::new(user_channel_id)
                        .delete_message(&ctx.http, MessageId::new(bot_message_id))
                        .await
                    {
                        eprintln!(
                            "[ERROR] Failed to delete DM message {}: {}",
                            user_channel_id, e
                        );
                    } else {
                        let query =
                            format!("DELETE FROM {} WHERE bot = ?", table_name);
                        let _ = sqlx::query(&query)
                            .bind(bot_message_id)
                            .execute(pool)
                            .await;
                    }
                }

                return;
            }

            let query = format!(
                "SELECT bot FROM {} WHERE user = ?",
                table_name
            );

            if let Ok(row) = sqlx::query(&query)
                .bind(message_id.get())
                .fetch_one(pool)
                .await
            {
                use sqlx::Row;

                let bot_message_id: u64 = row.get("bot");

                let query =
                    format!("DELETE FROM {} WHERE bot = ?", table_name);
                let _ = sqlx::query(&query)
                    .bind(bot_message_id)
                    .execute(pool)
                    .await;

                let embed = CreateEmbed::new()
                    .title("제거됨")
                    .color(0x2B001E);

                let bot_channel = ChannelId::new(bot_channel_id);

                let _ = bot_channel
                    .send_message(
                        &ctx.http,
                        CreateMessage::new()
                            .embed(embed)
                            .reference_message(
                                MessageReference::new(
                                    MessageReferenceKind::Default,
                                    bot_channel
                                )
                                .message_id(MessageId::new(bot_message_id))
                            )
                            .allowed_mentions(
                                CreateAllowedMentions::new().empty_users()
                            )
                    )
                    .await;
            }

            return;
        }
    }

    async fn message_update(&self, ctx: Context, _old: Option<Message>, _new: Option<Message>, event: MessageUpdateEvent) {
        if let Some(author) = &event.author {
            if author.id == BOT_USERID {
                return;
            }
        }

        let channel_id_u64 = event.channel_id.get();

        if let Some(guild_id) = event.guild_id {
            if guild_id.get() != GUILD_ID.get() {
                return;
            }

            let dm_pair = {
                let guard = self.dm_channels.lock().await;
                guard.iter()
                    .find(|&&(bot_ch, user_ch)| {
                        bot_ch == channel_id_u64 || user_ch == channel_id_u64
                    })
                    .copied()
            };

            if let Some((bot_channel_id, user_channel_id)) = dm_pair {
                let pool = er!(get_pool_tokidm().await, "[ERROR] DM message_update bot->user get_pool_tokidm()");
                let table_name = format!("c{}u{}", bot_channel_id, user_channel_id);
                if let Some(pos) = table_name.find('u') {
                    if let Ok(user_channel_id_u64) = table_name[pos + 1..].parse::<u64>() {
                        let query = format!(
                            "SELECT bot FROM {} WHERE user = ?",
                            table_name
                        );
                        
                        let result = sqlx::query(&query)
                            .bind(event.id.get())
                            .fetch_one(pool)
                            .await;

                        match result {
                            Ok(row) => {
                                let bot_message_id: u64 = row.get("bot");
                                let mut message = EditMessage::new();
                                    
                                if let Some(content) = event.content {
                                    message = message.content(content);
                                }
                
                                if let Some(attachments) = event.attachments {
                                    let mut attachmentsi = EditAttachments::new();
                                    for attachment in attachments {
                                        if let Ok(create_attachment) = CreateAttachment::url(&ctx.http, &attachment.url).await {
                                            attachmentsi = attachmentsi.add(create_attachment);
                                        }
                                    }
                                    message = message.attachments(attachmentsi);
                                }
                                if let Some(embeds) = event.embeds {
                                    if !embeds.is_empty() {
                                        let mut sendembeds = Vec::new();
                                        for embed in &embeds {
                                            sendembeds.push(embed.clone().into());
                                        }
                                        message = message.embeds(sendembeds);
                                    }   
                                }    
                                if let Some(components) = event.components {
                                    if !components.is_empty() {
                                        let mut pid = 0;
                                        let mut sendcomponents = Vec::new();
                                    
                                        for actionrow in &components {
                                            let mut action_row = Vec::new();
                                    
                                            for component in &actionrow.components {
                                                match component {
                                                    ActionRowComponent::Button(button) => {
                                                        let mut button_block: CreateButton = button.clone().into();
                                                        button_block = button_block.custom_id(format!("_{}", pid));
                                                        pid += 1;
                                                        action_row.push(CreateActionRow::Buttons(vec![button_block]));
                                                    }
                                                    _ => {}
                                                }
                                            }
                                    
                                            if !action_row.is_empty() {
                                                sendcomponents.extend(action_row);
                                            }
                                        }
                                    
                                        message = message.components(sendcomponents);
                                    }                                    
                                }

                                if let Err(e) = ChannelId::new(user_channel_id_u64).edit_message(&ctx.http, MessageId::new(bot_message_id), message).await {
                                    eprintln!("[ERROR] Failed to send DM to user {}: {}", user_channel_id_u64, e);
                                }
                            },
                            Err(_) => {}
                        }
                    }
                }
            }
        } else {
            let dm_pair = {
                let guard = self.dm_channels.lock().await;
                guard.iter()
                    .find(|&&(bot_ch, user_ch)| {
                        bot_ch == channel_id_u64 || user_ch == channel_id_u64
                    })
                    .copied()
            };

            if let Some((bot_channel_id, user_channel_id)) = dm_pair {
                let pool = er!(get_pool_tokidm().await, "[ERROR] DM message_update user->bot get_pool_tokidm()");
                let table_name = format!("c{}u{}", bot_channel_id, user_channel_id);

                if let Some(pos) = table_name.find('u') {
                    if let Ok(bot_channel_id_u64) = table_name[1..pos].parse::<u64>() {
                        let query = format!(
                            "SELECT bot FROM {} WHERE user = ?",
                            table_name
                        );
                        
                        let result = sqlx::query(&query)
                            .bind(event.id.get())
                            .fetch_one(pool)
                            .await;

                        match result {
                            Ok(row) => {
                                let bot_message_id: u64 = row.get("bot");
                                let mut message = EditMessage::new();
                                    
                                if let Some(content) = event.content {
                                    message = message.content(content);
                                }
                
                                if let Some(attachments) = event.attachments {
                                    let mut attachmentsi = EditAttachments::new();
                                    for attachment in attachments {
                                        if let Ok(create_attachment) = CreateAttachment::url(&ctx.http, &attachment.url).await {
                                            attachmentsi = attachmentsi.add(create_attachment);
                                        }
                                    }
                                    message = message.attachments(attachmentsi);
                                }
                                if let Some(embeds) = event.embeds {
                                    if !embeds.is_empty() {
                                        let mut sendembeds = Vec::new();
                                        for embed in &embeds {
                                            sendembeds.push(embed.clone().into());
                                        }
                                        message = message.embeds(sendembeds);
                                    }   
                                }    
                                if let Some(components) = event.components {
                                    if !components.is_empty() {
                                        let mut pid = 0;
                                        let mut sendcomponents = Vec::new();
                                    
                                        for actionrow in &components {
                                            let mut buttons = Vec::new();
                                            
                                            for component in &actionrow.components {
                                                if let ActionRowComponent::Button(button) = component {
                                                    let mut button_block: CreateButton = button.clone().into();
                                                    button_block = button_block.custom_id(format!("_{}",pid));
                                                    pid+=1;
                                                    buttons.push(button_block);
                                                }
                                            }
                
                                            if !buttons.is_empty() {
                                                sendcomponents.push(CreateActionRow::Buttons(buttons));
                                            }
                                        }                        
                                        message = message.components(sendcomponents);
                                    }
                                }

                                if let Err(e) = ChannelId::new(bot_channel_id_u64).edit_message(&ctx.http, MessageId::new(bot_message_id), message).await {
                                    eprintln!("[ERROR] Failed to send DM to user {}: {}", bot_channel_id_u64, e);
                                }
                            },
                            Err(_) => {}
                        }
                    }
                }
            }
        }
    }

    async fn message(&self, ctx: Context, msg: Message) {
        let localnow = Local::now();
        if msg.author.id == BOT_USERID {
            return;
        }
        /*
        if bot_userid_u64 == KKITUT_USERID && msg.content.starts_with("<#") { 
            return;
        }
        */

        if let Some(guild_id) = msg.guild_id {
            let is_trap = {
                let lock = self.trap_ids.lock().await;
                lock.get(&guild_id.get())
                    .and_then(|(channel_id, log_id)| {
                        if msg.channel_id == ChannelId::new(*channel_id) {
                            Some((*channel_id, *log_id))
                        } else {
                            None
                        }
                    })
            };

            if let Some((_, trap_log)) = is_trap {
                if let Some(perms) = msg.author_permissions(&ctx.cache) {
                    if perms.administrator() {
                        return;
                    }
                }

                let user_id = msg.author.id.get();
                let embedfooter = CreateEmbedFooter::new(&msg.author.name)
                .icon_url(
                    msg.author.avatar_url().unwrap_or_else(|| msg.author.default_avatar_url())
                );
                let ban_time = Local::now();
                if let Err(e) = guild_id.ban(&ctx.http, msg.author.id, 1).await {
                    eprintln!("[ERROR] Trap ban(): {:?}", e);
                    if let Some(log) = trap_log {
                        let embed = CreateEmbed::new()
                            .title("FAIL : Trap Ban")
                            .description(format!("{:?}", e))
                            .color(0xFFAAAA);
                        if let Err(e) = ChannelId::new(log).send_message(
                            &ctx.http,
                            CreateMessage::new().embed(embed)
                        ).await {
                            eprintln!("[ERROR] Trap error send_message(): {e:?}");
                        }
                    }
                    return;
                }

                if let Some(log) = trap_log {
                    let embed = CreateEmbed::new()
                        .title("GOTCHA")
                        .description(format!("<@{}>", user_id))
                        .color(0xFFAAAA)
                        .footer(embedfooter.clone())
                        .timestamp(ban_time);
        
                    let builder = CreateMessage::new().embed(embed);
                    if let Err(e) = ChannelId::new(log).send_message(&ctx.http, builder).await {
                        eprintln!("[ERROR] Trap send_message(): {:?}", e);
                    }
                }
                return;
            }

            let is_my_server = guild_id == GUILD_ID.get();

            let has_blacklist;
            {
                let guard = self.blacklist_guilds.lock().await;
                has_blacklist = guard.contains(&guild_id.get());
            }

            if !has_blacklist {
                let upmsg = msg.content.to_uppercase();

                if upmsg.starts_with("러스토키 ") || msg.content.starts_with("<\" ") {
                    let trimmed_msg_rustoki = if upmsg.starts_with("러스토키 ") {
                        upmsg[13..].trim().to_string()
                    } else {
                        String::new()
                    };
                
                    let trimmed_msg_quotes = if msg.content.starts_with("<\" ") {
                        upmsg[3..].trim().to_string()
                    } else {
                        String::new()
                    };
                
                    let message = if !trimmed_msg_rustoki.is_empty() {
                        trimmed_msg_rustoki.as_str()
                    } else if !trimmed_msg_quotes.is_empty() {
                        trimmed_msg_quotes.as_str()
                    } else {
                        return;
                    };
                
                    if message == "나" || message == "ME" {
                        if let Some(create_message) = get_response(&self.resp, &format!("<@{}>", msg.author.id.to_string())).await {
                            if let Err(e) = msg.channel_id.say(&ctx.http, create_message).await {
                                eprintln!("[ERROR] send_message(): {:?}", e);
                            }
                        } else {
                            if let Err(e) = msg.channel_id.say(&ctx.http, format!("{}{}",  msg.author.name, "은/는 DB에 없어")).await {
                                eprintln!("[ERROR] ? say(): {:?}", e);
                            }
                        }
                    } else {
                        if let Some(create_message) = get_response(&self.resp, &message).await {
                            if let Err(e) = msg.channel_id.say(&ctx.http, create_message).await {
                                eprintln!("[ERROR] send_message(): {:?}", e);
                            }
                        } else if let Some(spmsg) = get_special_response(&message, &ctx).await {
                            if spmsg.extra_note.starts_with("*sticker") && !is_my_server {
                                if let Err(e) = msg.channel_id.send_message(&ctx.http, CreateMessage::new().embed(CreateEmbed::new().description("(여기선 스티커를 보낼 수가 없어)").color(0xFFAAAA))).await {
                                    eprintln!("[ERROR] send_message(): {:?}", e);
                                }
                            } else {
                                if let Err(e) = msg.channel_id.send_message(&ctx.http, spmsg.to_create_message()).await {
                                    eprintln!("[ERROR] send_message(): {:?}", e);
                                }
                            }
                        } else {
                            if let Err(e) = msg.channel_id.say(&ctx.http, "?").await {
                                eprintln!("[ERROR] say(): {:?}", e);
                            }
                        }
                    }
                } else if msg.content == "<?" {
                    if let Err(e) = msg.reply_ping(&ctx.http, "도움말은 아직 없어").await {
                        eprintln!("[ERROR]  <? reply_ping(): {e:?}");
                    }
                    return;
                }
            }

            if msg.content.trim_start().starts_with("<!") {
                let allowed_ids = [
                    KKITUT_USERID.get(),
                    1202514875797213184,
                    507523158757212160,
                    412180095827181570,
                    682792713485418497,
                ];
                if !allowed_ids.contains(&msg.author.id.get()) {
                    return;
                }
            
                let rtcmd = msg.content.trim_start_matches("<!").trim_start();
            
                if rtcmd.is_empty() {
                    let help_msg =
"\
```
Rustoki Shell v0.1.0 -- made by Kkitut
A minimal shell-like interface for Debug or Admin purposes.

USAGE:
    <! <command> [options] [args]
    (Type `<! <command>` without arguments to see its usage.)

AVAILABLE COMMANDS:
    deunicode - Convert Unicode strings to ASCII-friendly output - https://github.com/kornelski/deunicode
    embed - Send the embed to the desired channel
    memory - Show memory usage
    resp - Manage response from database
    trap - Manage spam trap

Type <! to see this message again.
```
";
                    if let Err(e) = msg.channel_id.say(&ctx.http, help_msg).await {
                        eprintln!("[ERROR] <! say(): {:?}", e);
                    }
                    return;
                }

                if rtcmd.starts_with("memory") {
                    let result = match get_vmrss_mb().await {
                        Ok(mb) => format!("```SYS: {:.2} MB\n```", mb),
                        Err(e) => {
                            eprintln!("[ERROR] get_vmrss_mb(): {:?}", e);
                            "Failed to retrieve memory usage".to_string()
                        },
                    };

                    if let Err(e) = msg.channel_id.say(&ctx.http, result).await {
                        eprintln!("[ERROR] memory say(): {:?}", e);
                    }
                } else if let Some(rest) = rtcmd.strip_prefix("embed") {
                    let arg = rest.trim();
                    if arg.is_empty() {
                        let usage =
"\
```
embed <u64:ChannelId>
    [--authortext/-at \"<string>\"/me]
        ↳ string <= 256
    [--authorurl/-au <string:url>]
    [--authoravatar/-aa <string:url>/me]
    [--color/-c <string:RRGGBB>]
    [--description/-d \"<string>\"]
        ↳ string <= 4096
    [--field/-f \"<string:name>\" \"<string:value>\" <bool:inline(true|false)>]
        ↳ MAXCOUNT <= 25 | name <= 256 | value <= 1024
    [--footertext/-ft \"<string>\"/me]
        ↳ string <= 2048
    [--footericon/-fi <string:url/me>]
    [--image/-i <string:url>]
    [--thumbnail/-tn <string:url>/me]
    [--timestamp/-ts <string:ISO8601/UNIX>/now/utc]
    [--title/-t \"<string>\"]
        ↳ string <= 256
    [--url/-u <string:url>]

Special values:
    me   → use Your own info (avatar, name)
    now  → current timestamp
    utc  → current timestamp in UTC
```
";
                        if let Err(e) = msg.channel_id.say(&ctx.http, usage).await {
                            eprintln!("[ERROR] <! say(): {:?}", e);
                        }
                        return;
                    }

                    let mut parts = arg.split_whitespace();
                    let channel_id_str = match parts.next() {
                        Some(v) => v,
                        None => {
                            let _ = msg.reply_mention(&ctx.http, "ChannelId null").await;
                            return;
                        }
                    };

                    let channel_id: u64 = match channel_id_str.parse() {
                        Ok(v) => v,
                        Err(_) => {
                            let _ = msg.reply_mention(&ctx.http, "ChannelId must be a u64 integer").await;
                            return;
                        }
                    };

                    let mut author_text = None;
                    let mut author_url = None;
                    let mut author_avatar = None;
                    let mut color = None;
                    let mut description = None;
                    let mut fields = Vec::new();
                    let mut footer_text = None;
                    let mut footer_icon = None;
                    let mut image = None;
                    let mut thumbnail = None;
                    let mut timestamp = None;
                    let mut title = None;
                    let mut url = None;

                    let mut error_logs = Vec::new();
                    
                    let mut iter = parts.peekable();
                    while let Some(flag) = iter.next() {
                        match flag {
                            "--authortext" | "-at" => {
                                if let Some(first) = iter.next() {
                                    if first.eq_ignore_ascii_case("me") {
                                        author_text = Some(msg.author.name.clone());
                                    } else {
                                        match parse_quoted(&mut std::iter::once(first).chain(&mut iter), "--authortext", 256) {
                                            Ok(t) => author_text = Some(t),
                                            Err(e) => error_logs.push(e),
                                        }
                                    }
                                } else {
                                    error_logs.push(String::from("--authortext null"));
                                }
                            }
                            "--authorurl" | "-au" => {
                                if let Some(url) = iter.next() {
                                    author_url = Some(String::from(url));
                                } else {
                                    error_logs.push(String::from("--authorurl null"));
                                }
                            }
                                "--authoravatar" | "-aa" => {
                                if let Some(avatar) = iter.next() {
                                    if avatar == "me" {
                                        match msg.author.avatar {
                                            Some(hash) => {
                                                author_avatar = Some(format!(
                                                    "https://cdn.discordapp.com/avatars/{}/{}.png",
                                                    msg.author.id, hash
                                                ));
                                            }
                                            None => {
                                                author_avatar = Some(msg.author.default_avatar_url());
                                            }
                                        }
                                    } else {
                                        author_avatar = Some(String::from(avatar));
                                    }
                                } else {
                                    error_logs.push(String::from("--authoravatar null"));
                                }
                            }
                            "--color" | "-c" => {
                                if let Some(hex) = iter.next() {
                                    match u32::from_str_radix(hex, 16) {
                                        Ok(val) if hex.len() == 6 => {
                                            color = Some(val);
                                        }
                                        Ok(_) => {
                                            error_logs.push("color value must be 6 digit RRGGBB Hex".to_string());
                                        }
                                        Err(e) => {
                                            error_logs.push(format!("Invalid color value: {:?}", e));
                                        }
                                    }
                                } else {
                                    error_logs.push(String::from("--color null"));
                                }
                            }
                            "--description" | "-d" => {
                                match parse_quoted(&mut iter, "--description", 4096) {
                                    Ok(t) => description = Some(t),
                                    Err(e) => {
                                        error_logs.push(e);
                                    }
                                }
                            }
                            "--field" | "-f" => {
                                let name = match parse_quoted(&mut iter, "--field name", 256) {
                                    Ok(v) => v,
                                    Err(e) => {
                                        error_logs.push(e);
                                        continue;
                                    }
                                };

                                let value = match parse_quoted(&mut iter, "--field value", 1024) {
                                    Ok(v) => v,
                                    Err(e) => {
                                        error_logs.push(e);
                                        continue;
                                    }
                                };

                                let inline = match iter.next() {
                                    Some(v) if v.eq_ignore_ascii_case("true") => true,
                                    Some(v) if v.eq_ignore_ascii_case("false") => false,
                                    Some(_) => {
                                        error_logs.push(String::from("--field inline value must be true/false"));
                                        continue;
                                    }
                                    None => {
                                        error_logs.push(String::from("--field inline null"));
                                        continue;
                                    }
                                };

                                if fields.len() >= 25 {
                                    error_logs.push(String::from("field count must be less than 25"));
                                }

                                fields.push((name, value, inline));
                            }
                            "--footertext" | "-ft" => {
                                if let Some(first) = iter.next() {
                                    if first.eq_ignore_ascii_case("me") {
                                        footer_text = Some(msg.author.name.clone());
                                    } else {
                                        match parse_quoted(&mut iter, "--footertext", 2048) {
                                            Ok(t) => footer_text = Some(t),
                                            Err(e) => {
                                                error_logs.push(e);
                                            }
                                        }
                                    }
                                } else {
                                    error_logs.push(String::from("--footertext null"));
                                }
                            }
                            "--footericon" | "-fi" => {
                                if let Some(icon) = iter.next() {
                                    if icon.eq_ignore_ascii_case("me") {
                                        match msg.author.avatar {
                                            Some(hash) => {
                                                footer_icon = Some(format!(
                                                    "https://cdn.discordapp.com/avatars/{}/{}.png",
                                                    msg.author.id, hash
                                                ));
                                            }
                                            None => {
                                                footer_icon = Some(msg.author.default_avatar_url());
                                            }
                                        }
                                    } else {
                                        footer_icon = Some(String::from(icon));
                                    }
                                } else {
                                    error_logs.push(String::from("--footericon null"));
                                }
                            }
                            "--image" | "-i" => {
                                if let Some(url) = iter.next() {
                                    image = Some(url);
                                } else {
                                    error_logs.push(String::from("--image null"));
                                }
                            }
                            "--thumbnail" | "-tn" => {
                                if let Some(tmb) = iter.next() {
                                    thumbnail = Some(tmb);
                                } else {
                                    error_logs.push(String::from("--thumbnail null"));
                                }
                            }
                            "--timestamp" | "-ts" => {
                                if let Some(ts) = iter.next() {
                                    timestamp = if ts == "now" || ts == "utc" {
                                        Some(Utc::now())
                                    } else if let Ok(parsed_ts) = ts.parse::<DateTime<Utc>>() {
                                        Some(parsed_ts)
                                    } else if let Ok(unix_ts) = ts.parse::<i64>() {
                                        match DateTime::<Utc>::from_timestamp(unix_ts, 0) {
                                            Some(dt) => Some(dt),
                                            None => {
                                                error_logs.push(String::from("Invalid timestamp value"));
                                                continue;
                                            }
                                        }
                                    } else {
                                        error_logs.push(String::from("Invalid timestamp value"));
                                        continue;
                                    };
                                } else {
                                    error_logs.push(String::from("--timestamp null"));
                                }
                            }
                            "--title" | "-t" => {
                                match parse_quoted(&mut iter, "--title", 256) {
                                    Ok(t) => title = Some(t),
                                    Err(e) => {
                                        error_logs.push(e);
                                    }
                                }
                            }
                            "--url" | "-u" => {
                                if let Some(u) = iter.next() {
                                    url = Some(u);
                                } else {
                                    error_logs.push(String::from("--url null"));
                                }
                            }
                            unknown => {
                                error_logs.push(format!("Unknown args: {unknown}"));
                            }
                        }
                    }

                    if !error_logs.is_empty() {
                        let count = error_logs.len();
                        let mut reply_text = format!("```\n!! {} error{} occurred:\n", count, if count > 1 { "s" } else { "" });
                        reply_text.push_str(&error_logs.join("\n"));
                        reply_text.push_str("\n```");
                        let _ = msg.reply_mention(&ctx.http, reply_text).await;
                        return;
                    }

                    let mut embed = CreateEmbed::new();

                    if let Some(text) = author_text {
                        let mut author = CreateEmbedAuthor::new(text);

                        if let Some(url) = author_url {
                            author = author.url(url);
                        }

                        if let Some(avatar) = author_avatar {
                            author = author.icon_url(avatar);
                        }
                        embed = embed.author(author);
                    } else if author_url.is_some() || author_avatar.is_some() {
                        error_logs.push(String::from("!! --authorurl/--authoravatar requires --authortext"));
                    }

                    if let Some(c) = color {
                        embed = embed.color(c);
                    }

                    if let Some(desc) = description {
                        embed = embed.description(desc);
                    }

                    for (n, v, i) in fields.iter() {
                        embed = embed.field(n, v, *i);
                    }

                    if let Some(text) = footer_text {
                        let mut footer = CreateEmbedFooter::new(text);
                        if let Some(icon) = footer_icon {
                            footer = footer.icon_url(icon);
                        }
                        embed = embed.footer(footer);
                    } else if footer_icon.is_some() {
                        error_logs.push(String::from("!! --footericon requires --footertext"));
                    }

                    if let Some(img) = image {
                        embed = embed.image(img);
                    }

                    if let Some(tmb) = thumbnail {
                        if tmb == "me" {
                            embed = embed.thumbnail(msg.author.face());
                        } else {
                            embed = embed.thumbnail(tmb);
                        }
                    }

                    if let Some(ts) = timestamp {
                        embed = embed.timestamp(ts);
                    }

                    if let Some(t) = title {
                        embed = embed.title(t);
                    }

                    if let Some(u) = url {
                        embed = embed.url(u);
                    }

                    if let Err(e) = ChannelId::new(channel_id)
                        .send_message(&ctx.http, CreateMessage::new().embed(embed))
                        .await
                    {
                        eprintln!("[ERROR] <! send_message: {:?}", e);
                        let _ = msg.reply_mention(&ctx.http, format!("```\n!! Message send FAIL:\n{:?}```", e)).await;
                    }
                } else if let Some(rest) = rtcmd.strip_prefix("deunicode") {
                    let arg = rest.trim();
                    if arg.is_empty() {
                        let usage = "`deunicode [--upcase/-u] [--comma/-c] [--quote/-q] <string>`";
                        if let Err(e) = msg.channel_id.say(&ctx.http, usage).await {
                            eprintln!("[ERROR] <! say(): {:?}", e);
                        }
                        return;
                    }
            
                    let mut upcase = false;
                    let mut comma = false;
                    let mut quote = false;
                    let mut text = String::new();
            
                    for part in arg.split_whitespace() {
                        match part {
                            "--upcase" | "-u" => upcase = true,
                            "--comma"  | "-c" => comma = true,
                            "--quote"  | "-q" => quote = true,
                            _ => {
                                if !text.is_empty() {
                                    text.push(' ');
                                }
                                text.push_str(part);
                            }
                        }
                    }
            
                    let mut result = deunicode::deunicode(&text);
            
                    if upcase {
                        result = result.to_uppercase();
                    }
                    if quote {
                        result = result
                            .split_whitespace()
                            .map(|w| format!("\"{}\"", w))
                            .collect::<Vec<_>>()
                            .join(" ");
                    }
                    if comma {
                        result = result.split_whitespace().collect::<Vec<_>>().join(", ");
                    }
            
                    if let Err(e) = msg.channel_id.say(&ctx.http, result).await {
                        eprintln!("[ERROR] <! say(): {:?}", e);
                    }
                } else if rtcmd.starts_with("resp") {
                    let mut response;
                    let pool = er!(get_pool().await, "[RESP] get_pool()");
                    let arg = rtcmd.trim_start_matches("resp").trim();

                    if arg.is_empty() {
                        response = String::from(
"\
resp <subcommand> [arguments]

resp is a command that manages the response when <\" or 'Rustoki' is called.

Subcommands:
    update
        ↳ Reload all resp data from the database and synchronize memory
        ↳ Usage: resp update

    new_key <string:key>
        ↳ Create a new key with empty responses
        ↳ Fails if the key already exists
        ↳ Usage: resp new_key HELLO

    add_key <string:key> <string:key2>
    add_key <string:key> [\"<string:key2>\", ...]
        ↳ Add one or multiple keys under an existing key
        ↳ Duplicate keys are ignored and produce an error
        ↳ Usage:
            resp add_key HELLO GREET
            resp add_key HELLO [\"GREET\", \"WELCOME\"]

    add <string:key> \"<string:response>\"
    add <string:key> [\"<string:response>\", ...]
        ↳ Add single or multiple responses to a key
        ↳ Usage:
            resp add HELLO hi
            resp add HELLO [\"hi\", \"hey\", \"welcome\"]

    get_table <string:key>
        ↳ Show all responses for the specified key
        ↳ Usage: resp get_table HELLO

    remove <string:key> \"<string:response>\" [-m/--multi]
        ↳ Remove a response from a key
        ↳ If -m/--multi is provided, removes all matching responses
        ↳ Usage:
            resp remove HELLO \"hi\"
            resp remove HELLO \"hi\" -m

    remove_table <string:key> DOUBLECHECK [-f/--force]
        ↳ Remove the key and all associated responses
        ↳ DOUBLECHECK is required for confirmation
        ↳ -f/--force  Bypass additional checks (remove even if the key exists under another ID)
        ↳ Usage: resp remove_table \"hello\" DOUBLECHECK -f
Notes:
    - JSON arrays are supported for bulk operations (e.g., add or add_key)
    - Invalid JSON formats are ignored
    - Commands are case-sensitive
    - It contains an AUTOMATIC STRING PROCESSOR. You must use capital letters WITHOUT spaces or special characters
");
                    } else if arg.starts_with("update") {
                        if arg.trim() != "update" {
                            response = String::from("[ERR] Invalid arguments for 'update'");
                        } else { 
                            match RespStore::load_all(pool).await {
                                Ok(list) => {
                                    self.resp_store.update(list).await;
                                    response = String::from("resp updated");
                                }
                                Err(e) => {
                                    eprintln!("[RESP] load_all(): {:?}", e);
                                    response = format!("[ERR] update failed: {:?}", e);
                                }
                            }
                        }
                    } else if arg.starts_with("new_key") {
                        let rest = arg.trim_start_matches("new_key").trim();
                        let parts: Vec<String> = shell_words::split(rest).unwrap_or_default();

                        if parts.is_empty() {
                            response = String::from("[ERR] Missing argument for 'new_key'");
                        } else {
                            let mut key = parts[0].clone();
                            let auto = parts.iter().any(|s| s == "-a" || s == "--auto");

                            if auto {
                                key = key.to_uppercase().replace(' ', "");
                            }

                            if key.contains(' ') || key != key.to_uppercase() {
                                response = String::from("[ERR] 'new_key' must be uppercase and contain no spaces (use -a to auto-fix)");
                            } else {
                                match db_new_key(pool, &key).await {
                                    Ok(_) => response = String::from("new_key completed"),
                                    Err(e) => {
                                        eprintln!("[RESP] db_new_key(): {:?}", e);
                                        response = format!("[ERR] new_key failed: {:?}", e);
                                    }
                                }
                            }
                        }
                    } else if arg.starts_with("add_key") {
                        let rest = arg.trim_start_matches("add_key").trim();

                        let auto = rest.contains("-a") || rest.contains("--auto");
                        let rest_clean = rest.replace("-a", "").replace("--auto", "").trim().to_string();

                        if rest_clean.starts_with('[') {
                            let mut it = rest_clean.splitn(2, ']').map(str::trim);
                            let first_key_part = it.next();
                            let keys_json_part  = it.next();

                            if let (Some(fk_raw), Some(kj_raw)) = (first_key_part, keys_json_part) {
                                let mut first_key = fk_raw.trim_start_matches('"').trim_end_matches('"').to_string();
                                if auto {
                                    first_key = first_key.to_uppercase().replace(' ', "");
                                }

                                if first_key.contains(' ') || first_key != first_key.to_uppercase() {
                                    response = String::from("[ERR] 'add_key' first key must be uppercase and contain no spaces (use -a to auto-fix)");
                                } else {
                                    match serde_json::from_str::<Vec<String>>(kj_raw) {
                                        Ok(mut second_keys) => {
                                            if auto {
                                                for key in second_keys.iter_mut() {
                                                    *key = key.to_uppercase().replace(' ', "");
                                                }
                                            }
                                            let invalid = second_keys.iter().any(|k| k.contains(' ') || *k != k.to_uppercase());
                                            if invalid {
                                                response = String::from("[ERR] 'add_key' second keys must be uppercase and contain no spaces (use -a to auto-fix)");
                                            } else {
                                                match db_add_key_bulk(pool, &first_key, &second_keys).await {
                                                    Ok(_) => response = String::from("add_key (bulk) completed"),
                                                    Err(e) => {
                                                        eprintln!("[RESP] db_add_key_bulk(): {:?}", e);
                                                        response = format!("[ERR] add_key (bulk) failed: {:?}", e);
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => response = format!("[ERR] Invalid JSON format for 'add_key': {:?}", e),
                                    }
                                }
                            } else {
                                response = String::from("[ERR] Invalid arguments for 'add_key'");
                            }
                        } else {
                            let parts: Vec<&str> = rest_clean.splitn(2, ' ').collect();
                            if parts.len() == 2 {
                                let mut first_key = parts[0].to_string();
                                let mut second_key = parts[1].to_string();

                                if auto {
                                    first_key  = first_key.to_uppercase().replace(' ', "");
                                    second_key = second_key.to_uppercase().replace(' ', "");
                                }

                                if first_key.contains(' ') || first_key != first_key.to_uppercase() ||
                                second_key.contains(' ') || second_key != second_key.to_uppercase() {
                                    response = String::from("[ERR] 'add_key' keys must be uppercase and contain no spaces (use -a to auto-fix)");
                                } else {
                                    match db_add_key(pool, &first_key, &second_key).await {
                                        Ok(_) => response = String::from("add_key completed"),
                                        Err(e) => {
                                            eprintln!("[RESP] db_add_key(): {:?}", e);
                                            response = format!("[ERR] add_key failed: {:?}", e);
                                        }
                                    }
                                }
                            } else {
                                response = String::from("[ERR] Invalid arguments for 'add_key'");
                            }
                        }
                    } else if arg.starts_with("add") {
                        let rest = arg.trim_start_matches("add").trim();
                        let parts: Vec<String> = shell_words::split(rest).unwrap_or_default();

                        if parts.len() < 2 {
                            response = String::from("[ERR] Invalid arguments for 'add'");
                        } else {
                            let key = &parts[0].to_uppercase();
                            let value_str = rest[key.len()..].trim();

                            if value_str.starts_with('[') {
                                match serde_json::from_str::<Vec<String>>(value_str) {
                                    Ok(probs) => {
                                        match db_add_resp_bulk(pool, key, &probs).await {
                                            Ok(_) => response = String::from("add (bulk) completed"),
                                            Err(e) => {
                                                eprintln!("[RESP] db_add_resp_bulk(): {:?}", e);
                                                response = format!("[ERR] add (bulk) failed: {:?}", e);
                                            }
                                        }
                                    }
                                    Err(e) => response = format!("[ERR] Invalid JSON array for 'add': {:?}", e),
                                }
                            } else {
                                if !(value_str.starts_with('"') && value_str.ends_with('"')) {
                                    response = String::from("[ERR] Value must be quoted for single add");
                                } else {
                                    let value = &value_str[1..value_str.len()-1];
                                    match db_add_resp(pool, key, value).await {
                                        Ok(_) => response = String::from("add completed"),
                                        Err(e) => {
                                            eprintln!("[RESP] db_add_resp(): {:?}", e);
                                            response = format!("[ERR] add failed: {:?}", e);
                                        }
                                    }
                                }
                            }
                        }
                    } else if arg.starts_with("get_table") {
                        let rest = arg.trim_start_matches("get_table").trim();
                        let parts: Vec<String> = shell_words::split(rest).unwrap_or_default();

                        if parts.len() == 1 {
                            let key = &parts[0].to_uppercase();

                            match db_get_table(pool, key).await {
                                Ok(probs) => {
                                    let probs_json = serde_json::to_string(&probs).unwrap_or("[]".to_string());
                                    response = format!("Responses for key '{}':\n{}", key, probs_json);
                                }
                                Err(e) => {
                                    eprintln!("[RESP] db_get_table(): {:?}", e);
                                    response = format!("[ERR] get_table failed: {:?}", e);
                                }
                            }
                        } else {
                            response = String::from("[ERR] Invalid arguments for 'get_table'");
                        }
                    } else if arg.starts_with("remove_table") {
                        let rest = arg.trim_start_matches("remove_table").trim();
                        let parts: Vec<String> = shell_words::split(rest).unwrap_or_default();

                        if parts.is_empty() {
                            response = String::from("[ERR] Invalid arguments for 'remove_all'");
                        } else {
                            if !parts.contains(&"DOUBLECHECK".to_string()) {
                                response = String::from("[ERR] remove_all requires DOUBLECHECK confirmation");
                            } else {
                                let key = &parts[0];

                                let force = parts.iter().any(|s| s == "-f" || s == "--force");

                                match db_remove_table(pool, key, force).await {
                                    Ok(_) => response = String::from("remove_table completed"),
                                    Err(e) => {
                                        eprintln!("[RESP] db_remove_table(): {:?}", e);
                                        response = format!("[ERR] remove_table failed: {:?}", e);
                                    }
                                }
                            }
                        }
                    } else if arg.starts_with("remove") {
                        let rest = arg.trim_start_matches("remove").trim();
                        
                        let multi = rest.contains("-m") || rest.contains("--multi");
                        let temp = rest.replace("-m", "").replace("--multi", "");
                        let rest_clean = temp.trim();

                        let mut iter = rest_clean.splitn(2, ' ');
                        let key = iter.next().unwrap_or("").to_uppercase();
                        let target_str = iter.next().unwrap_or("").trim();

                        if key.is_empty() || target_str.is_empty() {
                            response = String::from("[ERR] Invalid arguments for 'remove'");
                        } else if !(target_str.starts_with('"') && target_str.ends_with('"')) {
                            response = String::from("[ERR] Target must be quoted with double quotes (\")");
                        } else {
                            let target_unquoted = &target_str[1..target_str.len()-1];

                            match db_remove_resp(pool, &key, target_unquoted, multi).await {
                                Ok(_) => response = String::from("remove completed"),
                                Err(e) => {
                                    eprintln!("[RESP] db_remove_resp(): {:?}", e);
                                    response = format!("[ERR] remove failed: {:?}", e);
                                }
                            }
                        }
                    } else {
                        response = String::from("[ERR] Unknown resp command");
                    }

                    response = format!("```\n{}\n```", response);
                    
                    if response.len() > 2000 {
                        response = String::from("[ERR] Response too long to send");
                    }

                    if let Err(e) = msg.channel_id.say(&ctx.http, response).await {
                        eprintln!("[ERROR] <! resp say(): {:?}", e);
                    }
                }
                return;
            }
            
            if is_my_server {
                if msg.content.starts_with("<&") && msg.author.id == KKITUT_USERID {
                    ctx.set_presence(Some(ActivityData::custom("자러가는 중")), OnlineStatus::Idle);
                    let embed_off = CreateEmbed::new()
                        .title("OFF")
                        .color(0x000000);
                    let builderoff = CreateMessage::new().embed(embed_off);
                    if let Err(e) = &CHANNELID_LOG_SECRET.send_message(&ctx.http, builderoff).await {
                        eprintln!("[ERROR] OFF send_message(): {e:?}");
                    }
                    println!("[SYS] Exiting");
                    println!("[SYS] changing stop flag");
                    self.stop_flag.store(true, Ordering::Relaxed);
                    println!("[SYS] sending stop notify");
                    self.stop_notify.notify_waiters();
                    println!("[SYS] Creating a waiting task");
                    tokio::spawn({ async move {
                            println!("[SYS] 10 sec left");
                            sleep(tokio::time::Duration::from_secs(10)).await;
                            println!("[SYS] Shutting down shard");
                            ctx.shard.shutdown_clean();
                            println!("[SYS] EXIT");
                            std::process::exit(0);
                        }
                    });
                } else {
                    let channel_id_u64 = msg.channel_id.get();

                    let dm_pair = {
                        let guard = self.dm_channels.lock().await;
                        guard.iter()
                            .find(|&&(bot_ch, user_ch)| {
                                bot_ch == channel_id_u64 || user_ch == channel_id_u64
                            })
                            .copied()
                    };

                    if let Some((_bot_channel_id, user_channel_id_u64)) = dm_pair {
                        let pool = er!(get_pool_tokidm().await, "[ERROR] DM get_pool_tokidm()");
                        let table_name = format!("c{}u{}", channel_id_u64, user_channel_id_u64);
                        let mut message = CreateMessage::new();
                        let mut is_not_empty = false;
                        if !msg.content.is_empty() {
                            message = message.content(&msg.content);
                            is_not_empty = true;
                        }
                        let mut attachments = Vec::new();
                        for attachment in &msg.attachments {
                            if let Ok(create_attachment) = CreateAttachment::url(&ctx.http, &attachment.url).await {
                                attachments.push(create_attachment);
                            }
                        }
                        if !attachments.is_empty() {
                            message = message.add_files(attachments);
                            is_not_empty = true;
                        }
                        let mut sticker_ids = Vec::new();
                        for sticker_item in &msg.sticker_items {
                            if let Ok(sticker) = sticker_item.id.to_sticker(&ctx.http).await {
                                if sticker.kind != StickerType::Guild {
                                    sticker_ids.push(sticker.id);
                                }
                            }
                        }
                        if !sticker_ids.is_empty() {
                            message = message.add_sticker_ids(sticker_ids);
                            is_not_empty = true;
                        }
                        if let Some(message_reference) = msg.message_reference {
                            if let Some(reference_message_id) = message_reference.message_id {
                                let query = format!(
                                    "SELECT bot, user FROM {} WHERE bot = ?",
                                    table_name
                                );
                                let mut result = sqlx::query(&query)
                                    .bind(reference_message_id.get())
                                    .fetch_one(pool)
                                    .await;
                                if result.is_err() {
                                    let query = format!(
                                        "SELECT bot, user FROM {} WHERE user = ?",
                                        table_name
                                    );
                                    result = sqlx::query(&query)
                                        .bind(reference_message_id.get())
                                        .fetch_one(pool)
                                        .await;
                                }
            
                                match result {
                                    Ok(row) => {
                                        let user_message_id: u64 = row.get("user");
                                        let bot_message_id: u64 = row.get("bot");
                                        let target_message_id = if user_message_id == reference_message_id.get() {
                                            bot_message_id
                                        } else {
                                            user_message_id
                                        };
        
                                        let reference = MessageReference::new(message_reference.kind, ChannelId::new(user_channel_id_u64)).message_id(MessageId::new(target_message_id));
                                        message = message.reference_message(reference);
                                        if !msg.mentions.iter().any(|mention| mention.id == BOT_USERID || mention.id == msg.author.id){
                                            message = message.allowed_mentions(CreateAllowedMentions::new().empty_users());
                                        }
                                    },
                                    Err(_) => {}
                                }
                            }
                        }
                        if !msg.embeds.is_empty() {
                            let mut sendembeds = Vec::new();
                            for embed in &msg.embeds {
                                sendembeds.push(embed.clone().into());
                            }
                            message = message.embeds(sendembeds);
                            is_not_empty = true;
                        }                 
                        if !msg.components.is_empty() {
                            let mut pid = 0;
                            let mut sendcomponents = Vec::new();
                        
                            for actionrow in &msg.components {
                                let mut buttons = Vec::new();
                                
                                for component in &actionrow.components {
                                    if let ActionRowComponent::Button(button) = component {
                                        let mut button_block: CreateButton = button.clone().into();
                                        button_block = button_block.custom_id(format!("_{}",pid));
                                        pid+=1;
                                        buttons.push(button_block);
                                    }
                                }
    
                                if !buttons.is_empty() {
                                    sendcomponents.push(CreateActionRow::Buttons(buttons));
                                    is_not_empty = true;
                                }
                            }                        
                            message = message.components(sendcomponents);
                        }

                        if !is_not_empty && msg.interaction_metadata.is_some() {
                            message = message.content("상대방이 사용한 앱의 응답을 기다리고 있어");
                            is_not_empty = true;
                        }
                
                        let is_err = if is_not_empty {
                            match ChannelId::new(user_channel_id_u64).send_message(&ctx.http, message).await {
                                Ok(sent_message) => {
                                    let query = format!(
                                        "INSERT INTO {} (bot, user) VALUES (?, ?)",
                                        table_name
                                    );
                            
                                    if let Err(e) = sqlx::query(&query)
                                        .bind(sent_message.id.get())
                                        .bind(msg.id.get()) 
                                        .execute(pool)
                                        .await {
                                            eprintln!("[ERROR] Failed to insert into TOKIDMtable_name: {:?}", e);
                                        }
                                    0
                                }
                                Err(e) => {
                                    eprintln!("[ERROR] Failed to send DM to user {}: {:?}", user_channel_id_u64, e);
                                    2
                                }
                            }
                        } else {
                            1
                        };
                        if is_err != 0 {
                            let title = if is_err == 1 { "Unsendable message" } else { "An internal error occurred" };
                            let embed = CreateEmbed::new()
                                .title(title)
                                .color(0x2B001E);
                            if let Err(e) = msg.channel_id.send_message(&ctx.http, CreateMessage::new().embed(embed).reference_message(MessageReference::new(
                                MessageReferenceKind::Default,
                                msg.channel_id
                            ).message_id(msg.id))).await {
                                eprintln!("[ERROR] Failed to send info at DM channel user {}: {:?}", user_channel_id_u64, e);
                            }
                        }
                    }
                }
            }
        } else {
            let timestamp = localnow.timestamp();
            let response_str;
            let upmsg = msg.content.to_uppercase();
            if upmsg.starts_with("러스토키 ") || msg.content.starts_with("<\" ") {
                let trimmed_msg_rustoki = if upmsg.starts_with("러스토키 ") {
                    upmsg[13..].trim().to_string()
                } else {
                    String::new()
                };
            
                let trimmed_msg_quotes = if msg.content.starts_with("<\" ") {
                    upmsg[3..].trim().to_string()
                } else {
                    String::new()
                };
            
                let message = if !trimmed_msg_rustoki.is_empty() {
                    trimmed_msg_rustoki.as_str()
                } else if !trimmed_msg_quotes.is_empty() {
                    trimmed_msg_quotes.as_str()
                } else {
                    return;
                };
            
                if message == "나" || message == "ME" {
                    if let Some(create_message) = get_response(&self.resp, &format!("<@{}>", msg.author.id.to_string())).await {
                        response_str = Some(create_message.clone());
                        if let Err(e) = msg.channel_id.say(&ctx.http, create_message).await {
                            eprintln!("[ERROR] send_message(): {:?}", e);
                        }
                    } else {
                        let formatted_response = format!("{}{}", msg.author.name, "은/는 DB에 없어");
                        response_str = Some(formatted_response.clone());
                        if let Err(e) = msg.channel_id.say(&ctx.http, formatted_response).await {
                            eprintln!("[ERROR] ? say(): {:?}", e);
                        }
                    }
                } else {
                    if let Some(create_message) = get_response(&self.resp, &message).await {
                        response_str = Some(create_message.clone());
                        if let Err(e) = msg.channel_id.say(&ctx.http, create_message).await {
                            eprintln!("[ERROR] send_message(): {:?}", e);
                        }
                    } else if let Some(spmsg) = get_special_response(&message, &ctx).await {
                        if spmsg.extra_note.starts_with("*sticker") {
                            response_str = Some("(여기선 스티커를 보낼 수가 없어)".to_string());
                            if let Err(e) = msg.channel_id.send_message(&ctx.http, CreateMessage::new().embed(CreateEmbed::new().description("(여기선 스티커를 보낼 수가 없어)").color(0xFFAAAA))).await {
                                eprintln!("[ERROR] send_message(): {:?}", e);
                            }
                        } else {
                            response_str = Some(spmsg.extra_note.clone());
                            if let Err(e) = msg.channel_id.send_message(&ctx.http, spmsg.to_create_message()).await {
                                eprintln!("[ERROR] send_message(): {:?}", e);
                            }
                        }
                    } else {
                        response_str = None;
                        if let Err(e) = msg.channel_id.say(&ctx.http, "?").await {
                            eprintln!("[ERROR] say(): {:?}", e);
                        }
                    }
                };
                
            } else {
                response_str = None;
            }

            let user_channel_id = msg.channel_id;
            let user_channel_id_u64 = user_channel_id.get();
            let user_id_u64 = msg.author.id.get();
            let table_name_op: Option<String> = {
                let dm_channels;
                {
                    let guard = self.dm_channels.lock().await;
                    dm_channels = guard.clone();
                }
                let mut found_table_name = None;
                for (bot_ch_u64, dm_ch_u64) in dm_channels.iter() {
                    if *dm_ch_u64 == user_channel_id_u64 {
                        found_table_name = Some(format!("c{}u{}", bot_ch_u64, dm_ch_u64));
                        break;
                    }
                }
                found_table_name
            };

            let pool = er!(get_pool_tokidm().await, "[ERROR] DM get_pool_tokidm()");

            let table_name = if let Some(ref table_name_op) = table_name_op {
                table_name_op.clone()
            } else {
                let new_channel = CreateChannel::new(format!("☆▰_dm_{}", user_id_u64))
                    .category(ChannelId::new(1348253746798399549))
                    .kind(ChannelType::Text)
                    .topic(format!("{} (<@{}>) <t:{}>", &msg.author.name, user_id_u64, timestamp));

                let channel_id;
                match GUILD_ID.create_channel(&ctx, new_channel).await {
                    Ok(guildchannel) => {
                        channel_id = guildchannel.id;
                    },
                    Err(e) => {
                        eprintln!("[ERROR] Failed to create DM log channel {}: {:?}", user_channel_id_u64, e);
                        return;
                    }
                }

                let table_name = format!("c{}u{}", channel_id.get(), msg.channel_id.get());
                let query = format!(
                    "CREATE TABLE IF NOT EXISTS {} (
                        bot BIGINT UNSIGNED NOT NULL,
                        user BIGINT UNSIGNED NOT NULL
                    )",
                    table_name
                );

                if let Err(e) = sqlx::query(&query).execute(pool).await {
                    eprintln!("[ERROR] Failed to create table {}: {:?}", table_name, e);
                }

                {
                    let mut dm_channels_lock = self.dm_channels.lock().await;
                    dm_channels_lock.push((channel_id.get(), user_id_u64));
                }

                table_name
            };
            
            if let Some(pos) = table_name.find('u') {
                if let Ok(bot_channel_id_u64) = table_name[1..pos].parse::<u64>() {
                    let bot_channel_id = ChannelId::new(bot_channel_id_u64);
                    let mut message = CreateMessage::new();
                    let mut is_not_empty = false;
                    if !msg.content.is_empty() {
                        if let Some(response_str) = response_str {
                            message = message.embed(CreateEmbed::new().title(format!("[SYS] >> {}", response_str)).description(&msg.content).color(0xFFAAAA));
                        } else {
                            message = message.content(&msg.content.replace('@', "＠"));
                        }
                        is_not_empty = true;
                    }
                    let mut attachments = Vec::new();
                    for attachment in &msg.attachments {
                        if let Ok(create_attachment) = CreateAttachment::url(&ctx.http, &attachment.url).await {
                            attachments.push(create_attachment);
                        }
                    }
                    if !attachments.is_empty() {
                        message = message.add_files(attachments);
                        is_not_empty = true;
                    }
                    let mut sticker_ids = Vec::new();
                    for sticker_item in &msg.sticker_items {
                        if let Ok(sticker) = sticker_item.id.to_sticker(&ctx.http).await {
                            if sticker.kind != StickerType::Guild {
                                sticker_ids.push(sticker.id);
                            }
                        }
                    }
                    if !sticker_ids.is_empty() {
                        message = message.add_sticker_ids(sticker_ids);
                        is_not_empty = true;
                    }
                    if let Some(message_reference) = msg.message_reference {
                        if let Some(reference_message_id) = message_reference.message_id {
                            let query = format!(
                                "SELECT bot, user FROM {} WHERE bot = ?",
                                table_name
                            );
                            let mut result = sqlx::query(&query)
                                .bind(reference_message_id.get())
                                .fetch_one(pool)
                                .await;
                            if result.is_err() {
                                let query = format!(
                                    "SELECT bot, user FROM {} WHERE user = ?",
                                    table_name
                                );
                                result = sqlx::query(&query)
                                    .bind(reference_message_id.get())
                                    .fetch_one(pool)
                                    .await;
                            }
        
                            match result {
                                Ok(row) => {
                                    let user_message_id: u64 = row.get("user");
                                    let bot_message_id: u64 = row.get("bot");
                                    let target_message_id = if user_message_id == reference_message_id.get() {
                                        bot_message_id
                                    } else {
                                        user_message_id
                                    };

                                    let reference = MessageReference::new(message_reference.kind, ChannelId::new(bot_channel_id_u64)).message_id(MessageId::new(target_message_id));
                                    message = message.reference_message(reference);
                                    if !msg.mentions.iter().any(|mention| mention.id == BOT_USERID || mention.id == msg.author.id){
                                        message = message.allowed_mentions(CreateAllowedMentions::new().empty_users());
                                    }
                                },
                                Err(_) => {}
                            }
                        }
                    }
                    if !msg.embeds.is_empty() {
                        let mut sendembeds = Vec::new();
                        for embed in &msg.embeds {
                            sendembeds.push(embed.clone().into());
                        }
                        message = message.embeds(sendembeds);
                        is_not_empty = true;
                    }                 
                    if !msg.components.is_empty() {
                        let mut pid = 0;
                        let mut sendcomponents = Vec::new();
                    
                        for actionrow in &msg.components {
                            let mut buttons = Vec::new();
                            
                            for component in &actionrow.components {
                                if let ActionRowComponent::Button(button) = component {
                                    let mut button_block: CreateButton = button.clone().into();
                                    button_block = button_block.custom_id(format!("_{}",pid));
                                    pid+=1;
                                    buttons.push(button_block);
                                }
                            }

                            if !buttons.is_empty() {
                                sendcomponents.push(CreateActionRow::Buttons(buttons));
                                is_not_empty = true;
                            }
                        }                        
                        message = message.components(sendcomponents);
                    }

                    if !is_not_empty && msg.interaction_metadata.is_some() {
                        message = message.content("상대방이 사용한 앱의 응답을 기다리고 있어");
                        is_not_empty = true;
                    }

                    let message_id = msg.id.get();
                    let is_err = if is_not_empty {
                        match bot_channel_id.send_message(&ctx.http, message).await {
                            Ok(sent_message) => {
                                let query = format!("INSERT INTO {} (bot, user) VALUES (?, ?)", table_name);
                                if let Err(e) = sqlx::query(&query)
                                    .bind(sent_message.id.get())
                                    .bind(msg.id.get())
                                    .execute(pool)
                                    .await {
                                    eprintln!("[ERROR] Failed to insert into TOKIDM table_name: {:?}", e);
                                }
                                0
                            }
                            Err(e) => {
                                eprintln!("[ERROR] Failed to send DM to channel {}: {:?}", msg.author.id.get(), e);
                                2
                            }
                        }
                    } else {
                        1
                    };
                    
                    if is_err != 0 {
                        let title = if is_err == 1 { "Unsendable message" } else { "An internal error occurred" };
                        let embed = CreateEmbed::new().title(title).color(0x2B001E);

                        if let Err(e) = msg.channel_id.send_message(&ctx.http, CreateMessage::new().embed(embed)
                            .reference_message(MessageReference::new(MessageReferenceKind::Default, msg.channel_id).message_id(msg.id))).await {
                            eprintln!("[ERROR] Failed to send info at DM channel user {}: {:?}", msg.author.id.get(), e);
                        }
                    
                        if let Err(e) = bot_channel_id.send_message(&ctx.http, CreateMessage::new().embed(CreateEmbed::new().description(format!("-# *[SYS] >> {}: {} *", title, message_id)).color(0x2B001E))).await {
                            eprintln!("[ERROR] Failed to send DM err to channel {}: {:?}", message_id, e);
                        }
                    }
                }
            }
        }
    }
    
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Command(command) => {
                let lng = command.locale.as_str() == "ko";
                let cmdname = command.data.name.as_str();
                if cmdname.starts_with("z_") && command.user.id != KKITUT_USERID {
                    if let Err(e) = command.create_response(
                        &ctx.http,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .embed(CreateEmbed::new().description(
                                    lng_trs(lng, "Only the person who developed me can handle this command", "나를 개발하는 사람만 이 명령어를 다룰 수 있어")
                                ).color(0xFFAAAA))
                                .allowed_mentions(CreateAllowedMentions::new().empty_users())
                        )
                    ).await {
                        eprintln!("[ERROR] Command create_response(): {:?}", e);
                    }
                    return;
                }
                match cmdname {
                    "spy_data" => {
                        let respstr;

                        if let Some(message_id) = command.data.target_id {
                            let route = Route::ChannelMessage {
                                channel_id: command.channel_id.get().into(),
                                message_id: message_id.get().into(),
                            };
                            let request = Request::new(route, LightMethod::Get);

                            match ctx.http.request(request).await {
                                Ok(response) => {
                                    match response.bytes().await {
                                        Ok(body_bytes) => {
                                            match serde_json::from_slice::<Value>(&body_bytes) {
                                                Ok(json_value) => {
                                                    let response_json = serde_json::to_string_pretty(&json_value).unwrap_or_default();
                                                    let mut txt = format!(
                                                        "```json\n{}```",
                                                        response_json.replace("`", "\\`").replace("@", "＠")
                                                    );

                                                    if txt.len() > 4096 {
                                                        txt = format!(
                                                            "{}: ({})",
                                                            lng_trs(lng, "Message is too long", "메시지가 너무 길어"),
                                                            txt.len()
                                                        );
                                                    }

                                                    respstr = txt;
                                                }
                                                Err(_) => {
                                                    respstr = lng_trs(lng, "Failed to parse JSON", "JSON 파싱에 실패했어").to_string();
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            respstr = lng_trs(lng, "Failed to read response body", "응답 본문 읽기에 실패했어").to_string();
                                        }
                                    }
                                }
                                Err(_) => {
                                    respstr = lng_trs(lng, "Message not found", "메시지를 찾을 수 없었어").to_string();
                                }
                            }
                        } else {
                            respstr = lng_trs(lng, "Unable to get message ID", "메시지 ID를 가져올 수 없었어").to_string();
                        }

                        if let Err(e) = command.create_response(
                            &ctx.http,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .embed(CreateEmbed::new().description(respstr).color(0xFFAAAA))
                                    .allowed_mentions(CreateAllowedMentions::new().empty_users())
                            )
                        ).await {
                            eprintln!("[ERROR] Command create_response(): {:?}", e);
                        }
                    },
                    "inquiry" => {
                        if let Err(e) = command.create_response(&ctx.http, CreateInteractionResponse::Message(CreateInteractionResponseMessage::new()
                            .embed(CreateEmbed::new().title(
                                lng_trs(lng,"please DM!","DM ㄱㄱ!"))
                                .description(lng_trs(lng,
                                    "Simply send a DM to the bot to use it!",
                                    "단순히 봇에게 DM을 보내서 이용해줘!")).color(0xFFAAAA)).ephemeral(true))).await {
                            eprintln!("[ERROR] Command create_response(): {:?}", e);
                        }
                    },
                    "spy_profile" => {
                        if let Err(e) = command.defer(&ctx.http).await {
                            eprintln!("[ERROR] Command defer(): {:?}", e);
                            return;
                        }

                        if let Some(opt) = command.data.options.get(0) {
                            if let Some(userid) = opt.value.as_user_id() {
                                let user_id = UserId::new(userid.into());

                                if let Ok(user) = user_id.to_user(&ctx.http).await {
                                    let username = user.name.clone();
                                    let mut embed = CreateEmbed::new()
                                        .title(userid.get().to_string())
                                        .color(0xFFAAAA)
                                        .timestamp(Timestamp::now());
                                    let global_display_name = user.global_name.clone().unwrap_or_else(|| username.clone());
                                    if let Some(guild_id) = command.guild_id {
                                        if let Ok(member) = guild_id.member(&ctx.http, user_id).await {
                                            if let Some(ref servername) = member.nick {
                                                embed = embed.description(format!("**{}:** `{}`\n**{}:** `{}`\n**{}:** `{}`",
                                                    lng_trs(lng,"Name","이름"), username, lng_trs(lng,"Global Name","전역 이름"), global_display_name, lng_trs(lng,"Server Name","서버 이름"), servername
                                                ));
                                            } else {
                                                embed = embed.description(format!("**{}:** `{}`\n**{}:** `{}`",
                                                    lng_trs(lng,"Name","이름"), username, lng_trs(lng,"Global Name","전역 이름"), global_display_name
                                                ));
                                            }
                                            let userfaceup = update_avatar_size(user.face().as_str());
                                            let thumbnail_url = if let Some(guild_avatar) = member.avatar_url() {
                                                if let Some(option) = command.data.options.get(1) {
                                                    if let Some(isserver) = option.value.as_bool() {
                                                        if isserver {update_avatar_size(guild_avatar.as_str())} else {userfaceup}
                                                    } else {
                                                        userfaceup
                                                    }
                                                } else {
                                                    userfaceup
                                                }
                                            } else {
                                                userfaceup
                                            };
                                            embed = embed.thumbnail(thumbnail_url);
                                        } else {
                                            match get_user_info(&ctx.http, user_id).await {
                                                Ok((username, avatar_url)) => {
                                                    embed = embed
                                                        .thumbnail(update_avatar_size(&avatar_url))
                                                        .description(format!("**{}:** `{}`", lng_trs(lng, "Name", "이름"), username));
                                                }
                                                Err(e) => {
                                                    eprintln!("[ERROR] get_user_info: {:?}", e);
                                                    embed = embed.description(lng_trs(lng, "User not found", "유저를 찾을 수 없었어"));
                                                }
                                            }
                                        }
                                    } 
                                    if let Err(e) = command.create_followup(&ctx.http, CreateInteractionResponseFollowup::new().add_embed(embed)).await {
                                        eprintln!("[ERROR] spy_profile response: {:?}", e);
                                    }
                                } else {
                                    if let Err(e) = command.create_followup(&ctx.http, CreateInteractionResponseFollowup::new().content(lng_trs(lng,"User not found","유저를 찾을 수 없었어"))).await {
                                        eprintln!("[ERROR] spy_profile response: {:?}", e);
                                    }
                                }
                            } else {
                                if let Err(e) = command.create_followup(&ctx.http, 
                                    CreateInteractionResponseFollowup::new()
                                        .content(lng_trs(lng,"User ID field is empty","유저 ID란이 비어있어"))
                                        .ephemeral(true)
                                ).await {
                                    eprintln!("[ERROR] Command create_response(): {:?}", e);
                                }
                            }
                        }
                    },
                    "birthday_set" | "birthday_remove" => {
                        let now =  Local::now().time();
                        if (now.hour() == 23 && now.minute() >= 59) || (now.hour() == 0 && now.minute() == 0) {
                            if let Err(e) = command.create_response(&ctx.http, 
                                CreateInteractionResponse::Message(
                                    CreateInteractionResponseMessage::new()
                                    .content(lng_trs(lng,"It is maintenance time","점검 시간이야"))
                                    .ephemeral(true)
                                )
                            ).await {
                                eprintln!("[ERROR] Command create_response(): {:?}", e);
                            }
                        }
                        if let Err(e) = command.defer_ephemeral(&ctx.http).await {
                            eprintln!("[ERROR] Command defer(): {:?}", e);
                            return;
                        }
                        let respstr;
                        let pool = er!(get_pool().await, "[ERROR] interaction birthday get_pool()");
                        let user_id = command.user.id.get();
                        if let Some(opt) = command.data.options.get(0) {
                            if let Some(mmdd) = opt.value.as_i64() {
                                let is_okuser = if let Some(record) = sqlx::query!(
                                    r#"
                                    SELECT data, time FROM birthday_user WHERE id = ?
                                    "#,
                                    user_id
                                ).fetch_optional(pool).await.unwrap() {
                                    if record.time.unwrap() < ( Local::now() - Duration::days(364)) {
                                        1
                                    } else {
                                        2
                                    }
                                } else {
                                    0
                                };
                                if is_okuser > 1 {
                                    respstr = lng_trs(lng, 
                                        "You can only set your birthday once a year. If a reset is required, please contact the administrator",
                                        "1년에 한번만 생일을 설정할 수 있어. 재설정이 필요하다면 담당자에게 문의해"
                                    );
                                } else {
                                    let month = (mmdd / 100) as u32;
                                    let day = (mmdd % 100) as u32;

                                    let is_wrong_month = !(1..=12).contains(&month);
                                    let is_wrong_day = !(1..=31).contains(&day);

                                    if is_wrong_month && is_wrong_day {
                                        respstr = lng_trs(lng, "Invalid date", "잘못된 날짜야");
                                    } else if is_wrong_day {
                                        respstr = lng_trs(lng, "Invalid day", "잘못된 일자야");
                                    } else if is_wrong_month {
                                        respstr = lng_trs(lng, "Invalid month", "잘못된 월이야");
                                    } else if month == 2 && day > 29 {
                                        respstr = lng_trs(lng, "February cannot exceed 29 days", "2월은 29일을 초과할 수 없어");
                                    } else if month == 2 && day == 29 {
                                        let suffix = if is_okuser == 1 { "_update" } else { "" };
                                        let bt1 = CreateButton::new(format!("birthday_leap_forward{}:{}", suffix, command.user.id.get().to_string()))
                                            .label("2/28")
                                            .style(ButtonStyle::Primary);
                                        let bt2 = CreateButton::new(format!("birthday_leap_keep{}:{}", suffix, command.user.id.get().to_string()))
                                            .label(lng_trs(lng, "KEEP", "유지"))
                                            .style(ButtonStyle::Secondary);
                                        let bt3 = CreateButton::new(format!("birthday_leap_backward{}:{}", suffix, command.user.id.get().to_string()))
                                            .label("3/1")
                                            .style(ButtonStyle::Primary);
                                        let embed = CreateEmbed::new()
                                            .title(lng_trs(lng, "February 29th", "2월 29일"))
                                            .color(0xFFAAAA)
                                            .description(lng_trs(lng, "February 29th is a leap year.\nYou can push the date forward or backward or **keep it as is**", "2월 29일은 윤년이야.\n날짜를 당기거나 밀거나 **그대로 진행**할 수 있어"));
                                        if let Err(e) = command.create_followup(&ctx.http, CreateInteractionResponseFollowup::new().add_embed(embed).button(bt1).button(bt2).button(bt3)).await {
                                            eprintln!("[ERROR] birthday_set create_followup(): {:?}", e);
                                        }
                                        return;
                                    } else {
                                        if is_okuser == 1 {
                                            let mmdd = format!("{:02}{:02}", month, day)
                                                .parse::<u16>()
                                                .unwrap();
                                    
                                            sqlx::query!(
                                                r#"
                                                UPDATE birthday_user SET data = ? WHERE id = ?
                                                "#,
                                                mmdd,
                                                user_id
                                            )
                                            .execute(pool)
                                            .await.unwrap();
                                    
                                            respstr = lng_trs(lng, "Birthday is updated", "생일이 업데이트되었어");
                                        } else {
                                            let mmdd = format!("{:02}{:02}", month, day)
                                                .parse::<u16>()
                                                .unwrap();
                                    
                                            sqlx::query!(
                                                r#"
                                                INSERT INTO birthday_user (id, data)
                                                VALUES (?, ?)
                                                "#,
                                                user_id,
                                                mmdd
                                            )
                                            .execute(pool)
                                            .await.unwrap();
                                    
                                            respstr = lng_trs(lng, "Birthday is set", "생일이 설정되었어");
                                        }
                                    }
                                }
                            } else {
                                respstr = lng_trs(lng, "Invalid value", "올바르지 않은 값이야");
                            }
                        } else {
                            if sqlx::query!(
                                r#"
                                SELECT time, unused FROM birthday_user WHERE id = ?
                                "#,
                                user_id
                            )
                            .fetch_optional(pool).await.unwrap().is_some() {
                                let record = sqlx::query!(
                                    r#"
                                    SELECT time, unused FROM birthday_user WHERE id = ?
                                    "#,
                                    user_id
                                )
                                .fetch_one(pool)
                                .await.unwrap();
                            
                                if record.time.unwrap() < ( Local::now() - Duration::days(364)) {
                                    sqlx::query!(
                                        r#"
                                        DELETE FROM birthday_user WHERE id = ?
                                        "#,
                                        user_id
                                    )
                                    .execute(pool)
                                    .await.unwrap();
                                    respstr = lng_trs(lng, "Removed successfully", "성공적으로 제거되었어");
                                } else {
                                    sqlx::query!(
                                        r#"
                                        UPDATE birthday_user SET unused = true WHERE id = ?
                                        "#,
                                        user_id
                                    )
                                    .execute(pool)
                                    .await.unwrap();
                                    respstr = lng_trs(lng, "Removed successfully", "성공적으로 제거되었어");
                                }
                            } else {
                                respstr = lng_trs(lng, "I couldn't find you", "너를 찾을 수 없었어");
                            }                            
                        }
                        if let Err(e) = command.create_followup(&ctx.http, CreateInteractionResponseFollowup::new().content(respstr)).await {
                            eprintln!("[ERROR] Command create_followup() at line {}: {:?}", line!(), e);
                        }
                    },
                    "ping" => {
                        let ts = command.id.created_at();
                        let elapsed_ms = Utc::now().timestamp_millis() - ts.timestamp_millis();

                        if let Err(e) = command.create_response(
                            &ctx.http,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .embed(CreateEmbed::new()
                                    .title(format!("{}ms", elapsed_ms))
                                    .color(0xFF8888))
                                    .allowed_mentions(CreateAllowedMentions::new().empty_users())
                            )
                        ).await {
                            eprintln!("[ERROR] Command create_response(): {:?}", e);
                        }
                    },
                    "delete_f_sel" | "delete_e_sel" => {
                        if let Err(e) = command.defer_ephemeral(&ctx.http).await {
                            eprintln!("[ERROR] Command defer(): {:?}", e);
                            return;
                        }

                        let respstr;
                        let user_id = command.user.id.get();
                        if let Some(message_id) = command.data.target_id {
                            if let Some(guild_id) = command.guild_id {
                                if let Some(ref member) = command.member {
                                    let perms = member.permissions.unwrap_or(Permissions::empty());
                                    if perms.manage_messages() {
                                        let channel_id = command.channel_id;

                                        if !(guild_id.get() == GUILD_ID.get() && {
                                            let guard = self.dm_channels.lock().await;
                                            guard.iter().any(|&(ch, _)| ch == channel_id.get())
                                        }) {
                                            let is_first = cmdname == "delete_f_sel";
                                            let id = message_id.get();
                                            let mut duplicate = false;

                                            {
                                                let mut lock = self.delete_messages.lock().await;

                                                if let Some(entry) = lock.iter_mut().find(|(uid, _, _, _)| *uid == user_id) {
                                                    if is_first {
                                                        if entry.2.0 == id {
                                                            duplicate = true;
                                                        } else {
                                                            entry.1.0 = id;
                                                            entry.1.1 = guild_id.get();
                                                            entry.1.2 = channel_id.get();
                                                            entry.3 = Local::now();
                                                        }
                                                    } else {
                                                        if entry.1.0 == id {
                                                            duplicate = true;
                                                        } else {
                                                            entry.2.0 = id;
                                                            entry.2.1 = guild_id.get();
                                                            entry.2.2 = channel_id.get();
                                                            entry.3 = Local::now();
                                                        }
                                                    }
                                                } else {
                                                    if is_first {
                                                        lock.push((user_id, (id, guild_id.get(), channel_id.get()), (0,0,0), Local::now()));
                                                    } else {
                                                        lock.push((user_id, (0,0,0), (id, guild_id.get(), channel_id.get()), Local::now()));
                                                    }
                                                }
                                            }

                                            if duplicate {
                                                respstr = lng_trs(lng, "Value is duplicated", "값이 중복이야").to_string();
                                            } else {
                                                respstr = format!(
                                                    "{}\nID: {}",
                                                    if is_first {
                                                        lng_trs(lng, "First Message Selected", "첫 메시지를 선택했어")
                                                    } else {
                                                        lng_trs(lng, "End Message Selected", "마지막 메시지를 선택했어")
                                                    },
                                                    message_id.to_message_id().link(channel_id, Some(guild_id))
                                                );
                                            }
                                        } else {
                                            respstr = lng_trs(lng, "Cannot use in admin DM channel", "관리자 DM 채널에선 사용할 수 없어").to_string();
                                        }
                                    } else {
                                        respstr = lng_trs(lng, "You do not have permission to manage messages", "넌 메시지 관리 권한이 없어").to_string();
                                    }
                                } else {
                                    respstr = lng_trs(lng, "Unable to get Your member info", "너의 맴버 정보를 가져올 수 없었어").to_string();
                                }
                            } else {
                                respstr = lng_trs(lng, "Only available in guilds", "길드에서만 사용 가능해").to_string();
                            }
                        } else {
                            respstr = lng_trs(lng, "Unable to get message ID", "메시지 ID를 가져올 수 없었어").to_string();
                        }

                        if let Err(e) = command.edit_response(
                            &ctx.http,
                            EditInteractionResponse::new()
                                .embed(CreateEmbed::new().description(respstr).color(0xFFAAAA))
                                .allowed_mentions(CreateAllowedMentions::new().empty_users())
                        ).await {
                            eprintln!("[ERROR] Command edit_response(): {:?}", e);
                        }
                    },
                    "delete_run" => {
                        if let Err(e) = command.defer_ephemeral(&ctx.http).await {
                            eprintln!("[ERROR] Command defer(): {:?}", e);
                            return;
                        }

                        let respstr;

                        if let Some(ref member) = command.member {
                            let perms = member.permissions.unwrap_or(Permissions::empty());
                            if perms.manage_messages() {
                                if let Some(bot_perms) = command.app_permissions {
                                    if bot_perms.manage_messages() {
                                        let user_id = command.user.id.get();
                                        let entry_opt = {
                                            let lock = self.delete_messages.lock().await;
                                            lock.iter().find(|(uid, _, _, _)| *uid == user_id).cloned()
                                        };
                                        if let Some(entry) = entry_opt {
                                            if entry.1.0 == 0 {
                                                respstr = lng_trs(lng, "First message not selected", "첫 메시지가 선택되지 않았어").to_string();
                                            } else if entry.2.0 == 0 {
                                                respstr = lng_trs(lng, "End message not selected", "끝 메시지가 선택되지 않았어").to_string();
                                            } else if entry.1.1 != entry.2.1 {
                                                respstr = lng_trs(lng, "Each message has a different guild", "각 메시지의 길드가 달라").to_string();
                                            } else if entry.1.2 != entry.2.2 {
                                                respstr = lng_trs(lng, "Each message has a different channel", "각 메시지의 채널이 달라").to_string();
                                            } else {
                                                let (start_id, end_id) = if entry.1.0 < entry.2.0 {
                                                    (entry.1.0, entry.2.0)
                                                } else {
                                                    (entry.2.0, entry.1.0)
                                                };
                                                let channel_id = ChannelId::new(entry.1.2);
                                                let max_loops = 20;
                                                let mut last_id = start_id;
                                                let mut messages_to_delete: Vec<MessageId> = Vec::new();
                                                let mut failed_error = None;

                                                match channel_id.message(&ctx.http, start_id).await {
                                                    Ok(msg) => messages_to_delete.push(msg.id),
                                                    Err(e) => failed_error = Some(e),
                                                }
                                                    
                                                let mut reached_max_loops = false;
                                                if failed_error.is_none() {
                                                    for i in 0..max_loops {
                                                        let messages = match channel_id
                                                            .messages(&ctx.http, GetMessages::new().after(last_id).limit(50))
                                                            .await
                                                        {
                                                            Ok(msgs) => msgs,
                                                            Err(e) => {
                                                                failed_error = Some(e);
                                                                break;
                                                            }
                                                        };

                                                        if messages.is_empty() {
                                                            break;
                                                        }

                                                        for message in &messages {
                                                            let message_id = message.id.get();
                                                            if message_id <= end_id {
                                                                messages_to_delete.push(message.id);
                                                            }
                                                            if message_id > last_id {
                                                                last_id = message_id;
                                                            }
                                                        }

                                                        if last_id >= end_id {
                                                            break;
                                                        }

                                                        if i + 1 == max_loops {
                                                            reached_max_loops = true;
                                                        }
                                                    }
                                                }
                                                        
                                                if let Some(e) = failed_error {
                                                    respstr = format!("{}: {}", lng_trs(lng, "Failed to fetch messages", "메시지 가져오기에 실패했어"), e);
                                                } else {
                                                    if messages_to_delete.is_empty() {
                                                        if reached_max_loops {
                                                            respstr = lng_trs(lng, "Too many messages to delete. (over) Please select a smaller range", "삭제할 메시지가 너무 많아. (1000 초과) 더 작은 범위를 선택해줘").to_string();
                                                        } else {
                                                            respstr = lng_trs(lng, "No messages to delete", "삭제할 메시지가 없어").to_string();
                                                        }
                                                    } else {
                                                        let mut delete_error = None;
                                                        for chunk in messages_to_delete.chunks(50) {
                                                            if let Err(e) = channel_id.delete_messages(&ctx.http, chunk.iter()).await {
                                                                delete_error = Some(format!("{}: {}", lng_trs(lng, "Failed to delete messages", "메시지 삭제에 실패했어"), e));
                                                                break;
                                                            }
                                                        }

                                                        if let Some(e) = delete_error {
                                                            respstr = e;
                                                        } else {
                                                            respstr = format!("{} {}", messages_to_delete.len(), lng_trs(lng, "messages deleted", "개의 메시지가 삭제되었어"));
                                                            let mut lock = self.delete_messages.lock().await;
                                                            lock.retain(|(uid, _, _, _)| *uid != user_id);
                                                        }
                                                    }
                                                }
                                            }
                                        } else {
                                            respstr = lng_trs(lng, "Not selected", "선택되지 않았어").to_string();
                                        }
                                    } else {
                                        respstr = lng_trs(lng, "I do not have permission to manage messages", "메시지 관리 권한이 없어").to_string();
                                    }
                                } else {
                                    respstr = lng_trs(lng, "Unable to check bot permissions", "내 권한을 확인할 수 없었어").to_string();
                                }
                            } else {
                                respstr = lng_trs(lng, "You do not have permission to manage messages", "넌 메시지 관리 권한이 없어").to_string();
                            }
                        } else {
                            respstr = lng_trs(lng, "Unable to get Your member info", "너의 맴버 정보를 가져올 수 없었어").to_string();
                        }

                        if let Err(e) = command.edit_response(
                            &ctx.http,
                            EditInteractionResponse::new()
                                .embed(CreateEmbed::new().description(respstr).color(0xFFAAAA))
                                .allowed_mentions(CreateAllowedMentions::new().empty_users())
                        ).await {
                            eprintln!("[ERROR] Command edit_response(): {:?}", e);
                        }
                    },
                    "dont_chat" => {
                        let respstr = if let Some(guild_id) = command.guild_id {
                            if let Some(ref member) = command.member {
                                if member.permissions.unwrap_or(Permissions::empty()).administrator() {
                                    if let Some(opt) = command.data.options.get(0) {
                                        if let Some(set) = opt.value.as_bool() {
                                            let pool = er!(get_pool().await, "[ERROR] interaction message_blacklist get_pool()");
                                            if set {
                                                match sqlx::query("DELETE FROM message_blacklist_guild WHERE id = ?")
                                                    .bind(guild_id.get())
                                                    .execute(pool)
                                                    .await
                                                {
                                                    Ok(_) => {
                                                        {
                                                            let mut guard = self.blacklist_guilds.lock().await;
                                                            guard.retain(|&id| id != guild_id.get());
                                                        }
                                                        lng_trs(lng, "I can now chat", "이제 대화할 수 있어")
                                                    },
                                                    Err(_) => lng_trs(lng, "Failed to allow chatting due to DB error", "대화 허용을 위한 DB 접근에 실패했어"),
                                                }
                                            } else {
                                                match sqlx::query("INSERT IGNORE INTO message_blacklist_guild (id) VALUES (?)")
                                                    .bind(guild_id.get())
                                                    .execute(pool)
                                                    .await
                                                {
                                                    Ok(_) =>{
                                                        {
                                                            let mut guard = self.blacklist_guilds.lock().await;
                                                            guard.push(guild_id.get());
                                                        }
                                                        lng_trs(lng, "I can no longer chat", "이제 대화할 수 없어")
                                                    },
                                                    Err(_) => lng_trs(lng, "Failed to block chatting due to DB error", "대화 차단을 위한 DB 접근에 실패했어"),
                                                }
                                            }
                                        } else {
                                            lng_trs(lng, "Option value invalid", "옵션 값이 잘못되었어")
                                        }
                                    } else {
                                        lng_trs(lng, "Option value missing", "옵션 값이 없어")
                                    }
                                } else {
                                    lng_trs(lng, "You are not an admin.", "넌 관리자가 아니야")
                                }
                            } else {
                                lng_trs(lng, "Unable to get Your member info", "너의 맴버 정보를 가져올 수 없었어")
                            }
                        } else {
                            lng_trs(lng, "There isn't a guild", "여긴 서버가 아니야")
                        };

                        if let Err(e) = command
                            .create_response(&ctx.http, CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .embed(CreateEmbed::new().description(respstr).color(0xFFAAAA))
                                    .allowed_mentions(CreateAllowedMentions::new().empty_users())
                                    .ephemeral(true)
                            ))
                            .await
                        {
                            eprintln!("[ERROR] Command create_response(): {:?}", e);
                        }
                    },
                    "call" => {
                        let respstr;
                        if let Some(opt) = command.data.options.get(0) {
                            if let Some(message) = opt.value.as_str() {
                                if message == "나" || message == "ME" {
                                    if let Some(rsp) = get_response(&self.resp, &format!("<@{}>", command.user.id.get().to_string())).await {
                                        respstr = rsp;
                                    } else {
                                        respstr = format!("{}{}", command.user.name, "은/는 DB에 없어");
                                    }
                                } else {
                                    if let Some(rsp) = get_response(&self.resp, &message).await {
                                        respstr = rsp;
                                    } else if let Some(spmsg) = get_special_response(&message, &ctx).await {
                                        if spmsg.extra_note.starts_with("*sticker") {
                                            respstr = "(여기선 스티커를 보낼 수가 없어)".to_string();
                                        } else {
                                            if let Err(e) = command
                                                .create_response(&ctx.http, CreateInteractionResponse::Message(spmsg.to_interaction_response()))
                                                .await
                                            {
                                                eprintln!("[ERROR] Command create_response(): {:?}", e);
                                            }
                                            return;
                                        }
                                    } else {
                                        respstr = "?".to_string();
                                    }
                                };     
                            } else {
                                respstr = lng_trs(lng, "Answer string is invalid", "질문 문자열에 오류가 있어").to_string();
                            }
                        } else {
                            respstr = lng_trs(lng, "Your answer is empty", "답변이 비었어").to_string();
                        }

                        if let Err(e) = command
                            .create_response(
                                &ctx.http,
                                CreateInteractionResponse::Message(
                                    CreateInteractionResponseMessage::new()
                                        .embed(
                                            CreateEmbed::new()
                                                .title(if respstr == "?" { "?" } else { "" })
                                                .description(if respstr == "?" { "" } else {&respstr})
                                                .color(0xFFAAAA),
                                        )
                                        .allowed_mentions(CreateAllowedMentions::new().empty_users()),
                                ),
                            )
                            .await
                        {
                            eprintln!("[ERROR] Command create_response(): {:?}", e);
                        }
                    },
                    "spy_channel" => {
                        let respstr;

                        let route = Route::Channel {
                            channel_id: command.channel_id.get().into(),
                        };
                        let request = Request::new(route, LightMethod::Get);

                        match ctx.http.request(request).await {
                            Ok(response) => {
                                match response.bytes().await {
                                    Ok(body_bytes) => {
                                        match serde_json::from_slice::<Value>(&body_bytes) {
                                            Ok(json_value) => {
                                                let response_json = serde_json::to_string_pretty(&json_value).unwrap_or_default();
                                                let mut txt = format!(
                                                    "```json\n{}```",
                                                    response_json.replace("`", "\\`").replace("@", "＠")
                                                );

                                                if txt.len() > 4096 {
                                                    txt = format!(
                                                        "{}: ({})",
                                                        lng_trs(lng, "Data is too long", "데이터가 너무 길어"),
                                                        txt.len()
                                                    );
                                                }

                                                respstr = txt;
                                            }
                                            Err(_) => {
                                                respstr = lng_trs(lng, "Failed to parse JSON", "JSON 파싱에 실패했어").to_string();
                                            }
                                        }
                                    }
                                    Err(_) => {
                                        respstr = lng_trs(lng, "Failed to read response body", "응답 본문 읽기에 실패했어").to_string();
                                    }
                                }
                            }
                            Err(_) => {
                                respstr = lng_trs(lng, "Channel not found", "채널을 찾을 수 없었어").to_string();
                            }
                        }

                        if let Err(e) = command.create_response(
                            &ctx.http,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .embed(CreateEmbed::new().description(respstr).color(0xFFAAAA))
                                    .allowed_mentions(CreateAllowedMentions::new().empty_users())
                            )
                        ).await {
                            eprintln!("[ERROR] Command create_response(): {:?}", e);
                        }
                    },
                    "z_trap_list" => {
                        let trap_list = {
                            self.trap_ids.lock().await.clone()
                        };
                        let mut list_string = String::new();
                        list_string.push_str("```\n");
                        list_string.push_str("+------------------+------------------+------------------+\n");
                        list_string.push_str("| guild_id         | channel_id       | log_id           |\n");
                        list_string.push_str("+------------------+------------------+------------------+\n");

                        for (guild_id, (channel_id, log_id)) in trap_list {
                            let log_str = match log_id {
                                Some(v) => v.to_string(),
                                None => "None".to_string(),
                            };

                            list_string.push_str(&format!(
                                "| {:<16} | {:<16} | {:<16} |\n",
                                guild_id, channel_id, log_str
                            ));
                        }

                        list_string.push_str("+------------------+------------------+------------------+\n```");

                        if let Err(e) = command.create_response(
                            &ctx.http,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .embed(CreateEmbed::new().description(list_string).color(0xFFAAAA))
                                    .allowed_mentions(CreateAllowedMentions::new().empty_users())
                                    .ephemeral(true)
                            )
                        ).await {
                            eprintln!("[ERROR] Command create_response(): {:?}", e);
                        }
                    },
                    "z_trap_set" => {
                        let respstr;
                        if let Some(guild_id) = command.guild_id {
                            if let Some(opt0) = command.data.options.get(0) {
                                if let Some(trigger_channel) = opt0.value.as_channel_id() {
                                    match ctx.http.get_channel(trigger_channel).await {
                                        Ok(tri_ch) => {
                                            match tri_ch {
                                                Channel::Guild(ch) => {
                                                    if ch.guild_id == guild_id {
                                                        if ch.kind == ChannelType::Text {
                                                            let log_channel;
                                                            if let Some(opt1) = command.data.options.get(1) {
                                                                if let Some(log_ch) = opt1.value.as_channel_id() {
                                                                    log_channel = Some(log_ch);
                                                                } else {
                                                                    if let Err(e) = command.create_response(
                                                                        &ctx.http,
                                                                        CreateInteractionResponse::Message(
                                                                            CreateInteractionResponseMessage::new()
                                                                                .embed(CreateEmbed::new().description(lng_trs(lng, "Log channel is invalid", "로그 채널이 잘못되었어")).color(0xFFAAAA))
                                                                                .allowed_mentions(CreateAllowedMentions::new().empty_users())
                                                                                .ephemeral(true)
                                                                        )
                                                                    ).await {
                                                                        eprintln!("[ERROR] Command create_response(): {:?}", e);
                                                                    }
                                                                    return;
                                                                }
                                                            } else {
                                                                log_channel = None;
                                                            }

                                                            if let Some(bot_perms) = command.app_permissions {
                                                                if bot_perms.ban_members() {
                                                                    if let Some(log_ch) = log_channel {
                                                                        let can_send_log = match ctx.http.get_channel(log_ch).await {
                                                                            Ok(log_ch) => {
                                                                                match log_ch {
                                                                                    Channel::Guild(ch) => {
                                                                                        if ch.guild_id == guild_id {
                                                                                            if ch.kind == ChannelType::Text {
                                                                                                match ctx.http.get_guild(guild_id).await {
                                                                                                    Ok(guild) => {
                                                                                                        match ctx.http.get_member(guild_id, BOT_USERID).await {
                                                                                                            Ok(bot) => {
                                                                                                                if guild.user_permissions_in(&ch, &bot).send_messages() {
                                                                                                                    None
                                                                                                                } else {
                                                                                                                    Some(lng_trs(lng, "I don't have permission to send messages from the log server", "로그 서버에서 메시지를 전송할 권한이 없어"))
                                                                                                                }
                                                                                                            }
                                                                                                            Err(e) => {
                                                                                                                eprintln!("[ERROR] Command get_member(): {:?}", e);
                                                                                                                Some(lng_trs(lng, "Failed to retrieve member information to view my permissions", "내 권한을 보기 위한 맴버 정보를 불러오는데 실패했어"))
                                                                                                            }
                                                                                                        }
                                                                                                    }
                                                                                                    Err(e) => {
                                                                                                        eprintln!("[ERROR] Command get_guild(): {:?}", e);
                                                                                                        Some(lng_trs(lng, "Failed to retrieve server information to view my permissions", "내 권한을 보기 위한 서버 정보를 불러오는데 실패했어"))
                                                                                                    }
                                                                                                }
                                                                                            } else {
                                                                                                Some(lng_trs(lng, "Log channel type is not a text channel", "로그 체널의 타입이 텍스트 체널이 아니야"))
                                                                                            }
                                                                                        } else {
                                                                                            Some(lng_trs(lng, "Log channel is not a channel of this server", "로그 채널이 이 서버의 체널이 아니야"))
                                                                                        }
                                                                                    }
                                                                                    _ => {
                                                                                        Some(lng_trs(lng, "Log channel is not a server channel", "로그 채널이 서버 체널이 아니야"))
                                                                                    }
                                                                                }
                                                                            }
                                                                            Err(e) => {
                                                                                eprintln!("[ERROR] Command get_channel(): {:?}", e);
                                                                                Some(lng_trs(lng, "Log channel is missing", "로그 채널을 찾을 수 없어"))
                                                                            }
                                                                        };
                                                                        if let Some(log) = can_send_log {
                                                                            if let Err(e) = command.create_response(
                                                                                &ctx.http,
                                                                                CreateInteractionResponse::Message(
                                                                                    CreateInteractionResponseMessage::new()
                                                                                        .embed(CreateEmbed::new().description(log).color(0xFFAAAA))
                                                                                        .allowed_mentions(CreateAllowedMentions::new().empty_users())
                                                                                        .ephemeral(true)
                                                                                )
                                                                            ).await {
                                                                                eprintln!("[ERROR] Command create_response(): {:?}", e);
                                                                            }
                                                                            return;
                                                                        }
                                                                    }

                                                                    let exists = {
                                                                        let lock = self.trap_ids.lock().await;
                                                                        lock.contains_key(&guild_id.get())
                                                                    };

                                                                    if !exists {
                                                                        match get_pool().await {
                                                                            Ok(pool) => {
                                                                                let trap_info = (guild_id.get(), trigger_channel.get(), log_channel.map(|c| c.get()));
                                                                                match sqlx::query!(
                                                                                    r#"
                                                                                    INSERT INTO trap (guild_id, channel_id, log_id)
                                                                                    VALUES (?, ?, ?)
                                                                                    "#,
                                                                                    trap_info.0,
                                                                                    trap_info.1,
                                                                                    trap_info.2
                                                                                )
                                                                                .execute(pool)
                                                                                .await {
                                                                                    Ok(_) => {
                                                                                        {
                                                                                            let mut lock = self.trap_ids.lock().await;
                                                                                            lock.insert(trap_info.0, (trap_info.1, trap_info.2));
                                                                                        }
                                                                                        respstr = lng_trs(lng, "Trap has been set", "함정이 설정되었어");
                                                                                    } 
                                                                                    Err(e) => {
                                                                                        eprintln!("[ERROR] Command sqlx::query!(): {:?}", e);
                                                                                        respstr = lng_trs(lng, "An error occurred while saving to the database", "DB에 저장하던 중에 오류가 발생했어");
                                                                                    }
                                                                                }
                                                                            }
                                                                            Err(e) => {
                                                                                eprintln!("[ERROR] Command get_pool(): {:?}", e);
                                                                                respstr = lng_trs(lng, "Failed to access DB for saving", "저장을 위한 DB 접근에 실패했어");
                                                                            }
                                                                        }
                                                                    } else {
                                                                        respstr = lng_trs(lng, "Trap has already been set", "함정이 이미 설정되어있어");
                                                                    }
                                                                } else {
                                                                    respstr = lng_trs(lng, "I do not have permission to ban members", "유저를 밴할 수 있는 권한이 없어");
                                                                }
                                                            } else {
                                                                respstr = lng_trs(lng, "Unable to check bot permissions", "내 권한을 확인할 수 없었어");
                                                            }
                                                        } else {
                                                            respstr = lng_trs(lng, "Trigger channel type is not a text channel", "발동 체널의 타입이 텍스트 체널이 아니야");
                                                        }
                                                    } else {
                                                        respstr = lng_trs(lng, "Trigger channel is not a channel of this server", "발동 채널이 이 서버의 체널이 아니야");
                                                    }
                                                }
                                                _ => {
                                                    respstr = lng_trs(lng, "Trigger channel is not a server channel", "발동 채널이 서버 체널이 아니야");
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("[ERROR] Command get_channel(): {:?}", e);
                                            respstr = lng_trs(lng, "Trigger channel is missing", "발동 채널을 찾을 수 없어");
                                        }
                                    }
                                } else {
                                    respstr = lng_trs(lng, "Trigger channel is invalid", "발동 채널이 잘못되었어");
                                }
                            } else {
                                respstr = lng_trs(lng, "Trigger channel is empty", "발동 채널이 비었어");
                            }
                        } else {
                            respstr = lng_trs(lng, "There isn't a guild", "여긴 서버가 아니야");
                        }

                        if let Err(e) = command.create_response(
                            &ctx.http,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .embed(CreateEmbed::new().description(respstr).color(0xFFAAAA))
                                    .allowed_mentions(CreateAllowedMentions::new().empty_users())
                                    .ephemeral(true)
                            )
                        ).await {
                            eprintln!("[ERROR] Command create_response(): {:?}", e);
                        }
                    },
                    "z_trap_remove" => {
                        let respstr;
                        if let Some(guild_id) = command.guild_id {
                            let guild_id_raw = guild_id.get();
                            let exists = {
                                let lock = self.trap_ids.lock().await;
                                lock.contains_key(&guild_id_raw)
                            };

                            if exists {
                                match get_pool().await {
                                    Ok(pool) => {
                                        match sqlx::query!(
                                            r#"
                                            DELETE FROM trap
                                            WHERE guild_id = ?
                                            "#,
                                            guild_id_raw
                                        )
                                        .execute(pool)
                                        .await {
                                            Ok(_) => {
                                                {
                                                    let mut lock = self.trap_ids.lock().await;
                                                    lock.remove(&guild_id_raw);
                                                }
                                                respstr = lng_trs(lng, "Trap has been deleted", "함정이 삭제되었어");
                                            }
                                            Err(e) => {
                                                eprintln!("[ERROR] Command sqlx::query!(): {:?}", e);
                                                respstr = lng_trs(lng, "An error occurred while deleting from the DB", "DB에서 삭제하던 중에 오류가 발생했어");
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("[ERROR] Command get_pool(): {:?}", e);
                                        respstr = lng_trs(lng, "Failed to access DB for deletion", "삭제를 위한 DB 접근에 실패했어");
                                    }
                                }
                            } else {
                                respstr = lng_trs(lng, "No trap set", "함정이 설정되어 있지 않아");
                            }
                        } else {
                            respstr = lng_trs(lng, "There isn't a guild", "여긴 서버가 아니야");
                        }

                        if let Err(e) = command.create_response(
                            &ctx.http,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .embed(CreateEmbed::new().description(respstr).color(0xFFAAAA))
                                    .allowed_mentions(CreateAllowedMentions::new().empty_users())
                                    .ephemeral(true)
                            )
                        ).await {
                            eprintln!("[ERROR] Command create_response(): {:?}", e);
                        }
                    },
                    _ => {
                        if let Err(e) = command.create_response(&ctx.http, CreateInteractionResponse::Message(CreateInteractionResponseMessage::new().content(lng_trs(lng,"command does not exist","존재하지 않는 명령어야")).ephemeral(true))).await {
                            eprintln!("[ERROR] Command create_response(): {:?}", e);
                        }
                    }
                }
            },
            Interaction::Component(component) => {
                let lng = component.locale.as_str() == "ko";
                let user_id = component.user.id.get();
                let parts: Vec<&str> = component.data.custom_id.as_str().split(':').collect();

                if parts.len() == 2 {
                    let user_id_orignal = parts[1].parse::<u64>().unwrap();
                    if user_id == user_id_orignal {
                        match parts[0] {
                            "birthday_leap_forward" | "birthday_leap_keep" | "birthday_leap_backward" | "birthday_leap_forward_update" | "birthday_leap_keep_update" | "birthday_leap_backward_update" => {
                                let pool = er!(get_pool().await, "[ERROR] interaction birthday get_pool()");
                                let user_id = component.user.id.get();
                                let mmdd: u16 = if component.data.custom_id == "birthday_leap_forward" || component.data.custom_id == "birthday_leap_forward_update" {
                                    228
                                } else if component.data.custom_id == "birthday_leap_keep" || component.data.custom_id == "birthday_leap_keep_update" {
                                    229
                                } else {
                                    301
                                };
        
                                let message = if component.data.custom_id.ends_with("_update") {
                                    lng_trs(lng, "Birthday is updated", "생일이 업데이트되었어")
                                } else {
                                    lng_trs(lng, "Birthday is set", "생일이 설정되었어")
                                };
        
                                sqlx::query!(
                                    r#"
                                    UPDATE birthday_user SET data = ? WHERE id = ?
                                    "#,
                                    mmdd,
                                    user_id
                                )
                                .execute(pool)
                                .await.unwrap();
        
                                let embed = CreateEmbed::new()
                                    .title(message)
                                    .color(0x00FF00);
        
                                if let Err(e) = component.create_response(&ctx.http, CreateInteractionResponse::UpdateMessage(
                                        CreateInteractionResponseMessage::new().embed(embed).components(vec![])
                                    )).await
                                {
                                    eprintln!("[ERROR] Button birthday_leap_: {:?}", e);
                                }
                            },
                            _ => {}
                        }       
                    }
                } else if component.data.custom_id.as_str() == "timeout_835743162131546132" {
                    let until = Timestamp::from_unix_timestamp((chrono::Utc::now() + chrono::Duration::seconds(5)).timestamp()).unwrap();
                    let result = GuildId::new(932966507548397619).edit_member(&ctx.http, UserId::new(835743162131546132), EditMember::new().disable_communication_until_datetime(until)).await;
                    let response_content = match result {
                        Ok(_) => "✅",
                        Err(e) => { 
                            eprintln!("[ERROR] Failed timeout_835743162131546132: {:?}", e);
                            "❌"
                        }
                    };
                    if let Err(e) = component.create_response(&ctx.http, CreateInteractionResponse::Message(CreateInteractionResponseMessage::new().content(response_content).ephemeral(true))).await {
                        eprintln!("[ERROR] Button timeout_835743162131546132: {:?}", e);
                    }
                } else if component.data.custom_id.starts_with("_") {
                    let embed = CreateEmbed::new()
                        .title(lng_trs(lng, "It's a dummy button", "더미 버튼이야"))
                        .color(0xFFAAAA);
                    if let Err(e) = component
                        .create_response(&ctx.http, CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new().add_embed(embed).components(vec![]).ephemeral(true)
                        ))
                        .await
                    {
                        eprintln!("[ERROR] Unknown error create_response(): {:?}", e);
                    }
                } else {
                    let embed = CreateEmbed::new()
                            .title(lng_trs(lng, "Unknown error", "알 수 없는 오류야"))
                            .color(0xFF0000);
                        if let Err(e) = component
                            .create_response(&ctx.http, CreateInteractionResponse::UpdateMessage(
                                CreateInteractionResponseMessage::new().add_embed(embed).components(vec![])
                            ))
                            .await
                        {
                            eprintln!("[ERROR] Unknown error create_response(): {:?}", e);
                        }
                }
            },
            _ => {}
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        let is_ready = self.is_ready.load(Ordering::Relaxed);

        if is_ready {
            #[cfg(debug_assertions)]
            {
                println!("{} ?Reconnected", ready.user.name);
            }        
            #[cfg(not(debug_assertions))]
            {
                println!("{} !Reconnected", ready.user.name);
            }
        } else {
            #[cfg(debug_assertions)]
            {
                println!("{} ??", ready.user.name);
            }        
            #[cfg(not(debug_assertions))]
            {
                println!("{} !!", ready.user.name);
            }
        }

        if BOT_USERID != ready.user.id {
            println!("Current User is not {}...", &BOT_USERID.get());
            return
        }
    
        if !is_ready {
            match GUILD_ID
                .set_commands(&ctx.http, vec![
                    CreateCommand::new("inquiry")
                        .description("Create a ticket")
                        .description_localized("ko", "티켓을 생성"),
                    /*
                    CreateCommand::new("stock_register")
                        .description("Register a stock user")
                        .description_localized("ko", "주식 사용자를 등록"),
                    CreateCommand::new("stock_balance")
                        .description("Check your balance")
                        .description_localized("ko", "자산을 조회"),
                    CreateCommand::new("stock_buy")
                        .description("Buy stocks")
                        .description_localized("ko", "주식을 매수"),
                    CreateCommand::new("stock_sell")
                        .description("Sell ​​stocks")
                        .description_localized("ko", "주식을 매도"),
                    CreateCommand::new("stock_rank")
                        .description("Check stock ranking (wallet balance)")
                        .description_localized("ko", "주식 순위를 확인 (지갑 잔액)"),
                    */
                    CreateCommand::new("birthday_set")
                        .description("set your birthday")
                        .description_localized("ko", "생일을 설정")
                        .add_option(
                            CreateCommandOption::new(CommandOptionType::Integer, "birthday", "MMDD")
                                .required(true)
                                .set_autocomplete(false),
                        ),
                    CreateCommand::new("birthday_remove")
                        .description("Remove your registered birthday")
                        .description_localized("ko", "등록된 생일을 제거"),
                ])
                .await {
                    Ok(_) => {},
                    Err(e) => {
                        eprintln!("[ERROR] Commands register(): {:?}", e);
                    },
                }

            match Command::set_global_commands(&ctx.http, vec![
                CreateCommand::new("spy_data")
                    .kind(CommandType::Message)
                    .integration_types(vec![InstallationContext::Guild, InstallationContext::User]),
                CreateCommand::new("spy_profile")
                    .description("View user's profile picture")
                    .description_localized("ko", "사용자의 프로필 사진을 조회")
                    .add_option(
                        CreateCommandOption::new(CommandOptionType::User, "id", "User ID")
                            .description_localized("ko", "사용자 ID")
                            .required(true)
                            .set_autocomplete(false),
                    )
                    .add_option(
                        CreateCommandOption::new(CommandOptionType::Boolean, "server", "Extracts images to server profile if exists")
                            .description_localized("ko", "존재할 경우 이미지를 서버 프로필로 추출")
                            .required(false)
                            .set_autocomplete(false),
                    )
                    .integration_types(vec![InstallationContext::Guild, InstallationContext::User]),
                CreateCommand::new("ping")
                    .description("Test the latency")
                    .description_localized("ko", "지연 시간을 테스트")
                    .integration_types(vec![InstallationContext::Guild, InstallationContext::User]),
                CreateCommand::new("delete_f_sel")
                    .kind(CommandType::Message)
                    .integration_types(vec![InstallationContext::Guild]),
                CreateCommand::new("delete_e_sel")
                    .kind(CommandType::Message)
                    .integration_types(vec![InstallationContext::Guild]),
                CreateCommand::new("delete_run")
                    .kind(CommandType::Message)
                    .integration_types(vec![InstallationContext::Guild]),
                CreateCommand::new("dont_chat")
                    .description("Whether or not to respond when the bot is called")
                    .description_localized("ko", "봇이 호출되었을 때 응답할지 여부")
                    .add_option(
                        CreateCommandOption::new(CommandOptionType::Boolean, "enabled", "Allow or Deny")
                            .description_localized("ko", "허용 / 비허용")
                            .required(true)
                            .set_autocomplete(false),
                    )
                    .integration_types(vec![InstallationContext::Guild]),
                CreateCommand::new("call")
                    .description("Talk to the bot")
                    .description_localized("ko", "봇과 대화")
                    .add_option(
                        CreateCommandOption::new(CommandOptionType::String, "string", "Your question")
                            .description_localized("ko", "질문할 내용")
                            .required(true)
                            .set_autocomplete(false),
                    )
                    .integration_types(vec![InstallationContext::Guild, InstallationContext::User]),
                CreateCommand::new("spy_channel")
                    .description("View current channel's Data")
                    .description_localized("ko", "현제 채널 데이터 조회"),
                CreateCommand::new("z_trap_list")
                    .description("List all traps")
                    .description_localized("ko", "모든 함정을 나열")
                    .integration_types(vec![InstallationContext::Guild]),
                CreateCommand::new("z_trap_set")
                    .description("Set a trap")
                    .description_localized("ko", "함정을 설정")
                    .add_option(
                        CreateCommandOption::new(CommandOptionType::Channel, "channel", "Trigger Channel")
                            .description_localized("ko", "발동 채널")
                            .required(true)
                            .set_autocomplete(false)
                    )
                    .add_option(
                        CreateCommandOption::new(CommandOptionType::Channel, "log", "Log Channel")
                            .description_localized("ko", "로그 채널")
                            .required(false)
                            .set_autocomplete(false)
                    )
                    .integration_types(vec![InstallationContext::Guild]),
                CreateCommand::new("z_trap_remove")
                    .description("Remove a trap")
                    .description_localized("ko", "함정을 제거")
                    .integration_types(vec![InstallationContext::Guild]),

            ])
            .await {
                Ok(_) => {},
                Err(e) => {
                    eprintln!("[ERROR] Global Commands register(): {:?}", e);
                },
            }
        }

        let embed_on;
        if is_ready {
            #[cfg(debug_assertions)]
            {
                embed_on = CreateEmbed::new()
                .title("DEBUG RECONNECTED")
                .color(0x888888);
            }        
            #[cfg(not(debug_assertions))]
            {
                embed_on = CreateEmbed::new()
                .title("RECONNECTED")
                .color(0x0000FF);
            }
        } else {
            #[cfg(debug_assertions)]
            {
                embed_on = CreateEmbed::new()
                .title("DEBUG")
                .color(0x888888);
            }        
            #[cfg(not(debug_assertions))]
            {
                embed_on = CreateEmbed::new()
                .title("ON")
                .color(0x0000FF);
            }
        }
        
        let builderon = CreateMessage::new().embed(embed_on);
        if let Err(e) = &CHANNELID_LOG_SECRET.send_message(&ctx.http, builderon).await {
            eprintln!("[ERROR] ON send_message(): {:?}", e);
        }

        ctx.set_presence(Some(ActivityData::custom("노닥거리는 중")), OnlineStatus::Online);
        
        if !is_ready {
            self.is_ready.store(true, Ordering::Relaxed);

            let sfc0 = self.stop_flag.clone();
            let snc0 = self.stop_notify.clone();
            
            let http0 = ctx.http.clone();

            let pool = er!(get_pool().await, "[ERROR] ready get_pool()");

            tokio::spawn({
                let pool = pool.clone();
                let birthday_id = RoleId::new(1299153941623341056);
                let birth_channel = ChannelId::new(1415398554700349592);
                
                async move {
                    loop {
                        let mut birth_users: Vec<Member> = Vec::new();
                        let now_local = Local::now();
                        let today = (now_local.month() as u16) * 100 + now_local.day() as u16;
                        let birth_today_id_raw = sqlx::query!(
                            r#"
                            SELECT id, data, time, unused 
                            FROM birthday_user 
                            WHERE data = ?
                            "#,
                            today
                        )
                        .fetch_all(&pool)
                        .await
                        .unwrap_or_default();

                        if !birth_today_id_raw.is_empty() {
                            println!("[SYS] Birthday User(s) Found: {}", birth_today_id_raw.len());

                            let mut user_ids_today: Vec<u64> = Vec::new();

                            for row in birth_today_id_raw {
                                if let Some(time) = row.time {
                                    if row.unused == Some(0) {
                                        user_ids_today.push(row.id);
                                    }

                                    let now_utc = Utc::now();
                                    if row.unused.is_some() && (now_utc - time).num_days() > 364 {
                                        sqlx::query!(
                                            "DELETE FROM birthday_user WHERE id = ?",
                                            row.id
                                        )
                                        .execute(&pool)
                                        .await
                                        .ok();
                                    }
                                }
                            }
                            
                            match  GUILD_ID.to_partial_guild(&http0).await {
                                Ok(guild) => {
                                    let mut falled_user_ids = Vec::new();
                                    for user_id in user_ids_today {
                                        match guild.member(&http0, user_id).await {
                                            Ok(member) => {
                                                if !member.roles.contains(&birthday_id) {
                                                    birth_users.push(member);
                                                }
                                            }

                                            Err(e) => {
                                                let mut should_delete = true;

                                                if let serenity::all::Error::Http(http_err) = &e {
                                                    if let serenity::all::HttpError::UnsuccessfulRequest(resp) = http_err {
                                                        let status = resp.status_code.as_u16();
                                                        if status >= 500 || status == 429 {
                                                            should_delete = false;
                                                        }
                                                    }
                                                }

                                                if should_delete {
                                                    sqlx::query!(
                                                        "DELETE FROM birthday_user WHERE id = ?",
                                                        user_id
                                                    )
                                                    .execute(&pool)
                                                    .await
                                                    .ok();
                                                    
                                                    falled_user_ids.push(user_id);
                                                }
                                            }
                                        }
                                    }

                                    if !falled_user_ids.is_empty() {
                                        println!("[SYS] Birthday User(s) Removed (Left the server): {}", falled_user_ids.len());
                                    }

                                    if birth_users.len() > 0 {
                                        _ = birth_channel.broadcast_typing(&http0).await;
                                        
                                        let mut success_users_id = Vec::new();
                                        for user in &birth_users {
                                            match user.add_role(&http0, birthday_id).await {
                                                Ok(_) => {
                                                    success_users_id.push(user.user.id);
                                                    println!("[SYS] Birthday Role Added: {} ({})", user.user.id.get(), user.user.name);
                                                }
                                                Err(e) => {
                                                    eprintln!("[ERROR] Birthday add_role(): {:?} for {} ({})", e, user.user.name, user.user.id.get());
                                                }
                                            }
                                        }

                                        if !success_users_id.is_empty() {
                                            let mut lines = String::new();

                                            for user_id in &success_users_id {
                                                lines.push_str(&format!("- <@{}>\n", user_id));
                                            }

                                            let header = format!(
                                                "<@&1299153941623341056>\n🎉 ({})\n",
                                                success_users_id.len()
                                            );

                                            let full_content = format!("{}{}", header, lines);

                                            let content = if full_content.len() > 2000 {
                                                format!(
                                                    "{}(Too many people's birthdays!)",
                                                    header
                                                )
                                            } else {
                                                full_content
                                            };

                                            let birth_message = CreateMessage::new().content(content);

                                            if let Err(e) = birth_channel
                                                .send_message(&http0, birth_message)
                                                .await {
                                                    eprintln!("[ERROR] Birthday send_message(): {:?}", e);
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("[ERROR] Birthday GUILD_ID.to_partial_guild(): {:?}", e);
                                }
                            }
                        }

                        let now = Local::now().naive_local();

                        let mut target = Local::now()
                            .date_naive()
                            .and_hms_opt(0, 0, 0)
                            .unwrap();
                        if now >= target {
                            target += Duration::days(1);
                        }
                        let duration = target
                            .signed_duration_since(now)
                            .to_std()
                            .unwrap();

                        tokio::select! {
                            _ = sleep(duration) => {}
                            _ = snc0.notified() => {
                                if sfc0.load(Ordering::Relaxed) {
                                    println!("[SYS] Birth task Shutdown");
                                    break;
                                }
                            }
                        }

                        for user in &birth_users {
                            user.remove_role(&http0, birthday_id).await.unwrap();
                        }
                        birth_users.clear();
                    }
                }
            });

            let sfc1 = self.stop_flag.clone();
            let snc1 = self.stop_notify.clone();

            let http1 = ctx.http.clone();
            let rainbow_id = RoleId::new(1363166115165110513);
            tokio::spawn(async move {
                let colors: [u32; 12] = [
                    0xFF0000, 0xFF8000, 0xFFFF00, 0x80FF00,
                    0x00FF00, 0x00FF80, 0x00FFFF, 0x0080FF,
                    0x0000FF, 0x8000FF, 0xFF00FF, 0xFF0080,
                ];

                let mut index = 0;

                loop {
                    let rolebuilder = EditRole::new().colour(Color::from(colors[index]));
                    if let Err(e) = GUILD_ID.edit_role(&http1, rainbow_id, rolebuilder).await {
                        eprintln!("[ERROR] GUILD_ID edit_role color: {:?}", e);
                    }

                    index = (index + 1) % colors.len();

                    tokio::select! {
                        _ = sleep(tokio::time::Duration::from_secs(240)) => {}
                        _ = snc1.notified() => {
                            if sfc1.load(Ordering::Relaxed) {
                                println!("[SYS] Rainbow task Shutdown");
                                break;
                            }
                        }
                    }
                }
            });

            let sfc2 = self.stop_flag.clone();
            let snc2 = self.stop_notify.clone();

            let http2 = ctx.http.clone();
            tokio::spawn(async move {
                loop {
                    let today_naive = Local::now().date_naive();
                    let next_midnight = (today_naive + Duration::days(7))
                        .and_hms_opt(0, 0, 0)
                        .unwrap();

                    let remaining_time = next_midnight.signed_duration_since(Local::now().naive_local());
                    let duration = remaining_time
                        .to_std()
                        .unwrap_or_else(|_| std::time::Duration::from_secs(7 * 24 * 3600));

                    tokio::select! {
                        _ = sleep(duration) => {}
                        _ = snc2.notified() => {
                            if sfc2.load(Ordering::Relaxed) {
                                println!("[SYS] RoleGC task Shutdown");
                                break;
                            }
                        }
                    }

                    println!("[SYS] RoleGC RUN");

                    let builder = CreateMessage::new()
                        .embed(CreateEmbed::new()
                        .title("RoleGC RUN")
                        .color(0xFFAAAA)
                    );
                    if let Err(e) = CHANNELID_LOG_SECRET.send_message(&http2, builder).await {
                        eprintln!("[ERROR] Failed to send RoleGC RUN message: {:?}", e);
                    }

                    let mut manage_roles = Vec::new();
                    match GUILD_ID.roles(&http2).await {
                        Ok(roles) => {
                            for (role_id, role) in roles {
                                if role.name.starts_with(':') {
                                    manage_roles.push((role_id, role.name));
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("[ERROR] Failed to fetch roles: {:?}", e);
                            let builder = CreateMessage::new()
                                .embed(CreateEmbed::new()
                                .title("RoleGC FAIL")
                                .description(format!("Failed to fetch roles\n```{:?}```", e))
                                .color(0xFF4444)
                            );
                            if let Err(e) = CHANNELID_LOG_SECRET.send_message(&http2, builder).await {
                                eprintln!("[ERROR] Failed to send RoleGC FAIL message: {:?}", e);
                            }
                            return;
                        }
                    }

                    match GUILD_ID.members(&http2, None, None).await {
                        Ok(members) => {
                            manage_roles.retain(|(role_id, _name)| {
                                !members.iter().any(|member| member.roles.contains(role_id))
                            });
                        }
                        Err(e) => {
                            eprintln!("[ERROR] Failed to fetch members: {:?}", e);
                            let builder = CreateMessage::new()
                                .embed(CreateEmbed::new()
                                .title("RoleGC FAIL")
                                .description(format!("Failed to fetch members\n```{:?}```", e))
                                .color(0xFF4444)
                            );
                            if let Err(e) = CHANNELID_LOG_SECRET.send_message(&http2, builder).await {
                                eprintln!("[ERROR] Failed to send RoleGC FAIL message: {:?}", e);
                            }
                            return;
                        }
                    }

                    let mut deleted_role_names = Vec::new();
                    let mut failed_role_names = Vec::new();
                    for role_id in manage_roles {
                        if let Err(e) = GUILD_ID.delete_role(&http2, role_id.0).await {
                            eprintln!("[ERROR] Failed to delete role {}: {:?}", role_id.0, e);
                            failed_role_names.push(role_id.1);
                        } else {
                            deleted_role_names.push(role_id.1);
                        }
                    }

                    let end_message = match (deleted_role_names.is_empty(), failed_role_names.is_empty()) {
                        (true, true) => {
                            println!("[SYS] No roles processed");
                            String::from("```No roles deleted or failed```")
                        }
                        (false, true) => {
                            let roles_str = deleted_role_names.iter().map(|r| format!("  {}", r)).collect::<Vec<_>>().join("\n");
                            println!("[SYS] [Deleted roles]\n{}", roles_str);
                            format!("```[Deleted roles]\n{}\n```", roles_str)
                        }
                        (true, false) => {
                            let roles_str = failed_role_names.iter().map(|r| format!("  {}", r)).collect::<Vec<_>>().join("\n");
                            println!("[SYS] [Failed roles]\n{}", roles_str);
                            format!("```[Failed roles]\n{}\n```", roles_str)
                        }
                        (false, false) => {
                            let ok_str = deleted_role_names.iter().map(|r| format!("  {}", r)).collect::<Vec<_>>().join("\n");
                            let fail_str = failed_role_names.iter().map(|r| format!("  {}", r)).collect::<Vec<_>>().join("\n");
                            println!("[SYS] [Deleted roles]\n{}\n[Failed roles]\n{}", ok_str, fail_str);
                            format!("```[Deleted roles]\n{}\n\n[Failed roles]\n{}\n```", ok_str, fail_str)
                        }
                    };

                    let builder = CreateMessage::new()
                        .embed(CreateEmbed::new()
                            .title("RoleGC RESULT")
                            .description(end_message)
                            .color(0xFFAAAA)
                        );

                    if let Err(e) = CHANNELID_LOG_SECRET.send_message(&http2, builder).await {
                        eprintln!("[ERROR] Failed to send RoleGC END message: {:?}", e);
                    }
                }
            });
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    if let Err(e) = TOKEN.set(env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN not set in .env file")) {
        eprintln!("[ERROR] TOKEN set(): {:?}", e);
        return;
    }
    if let Err(e) = DATABASE_URL.set(env::var("DATABASE_URL").expect("DATABASE_URL not set in .env file")) {
        eprintln!("[ERROR] DATABASE_URL set(): {:?}", e);
        return;
    }
    if let Err(e) = DATABASE_URL_TOKIDM.set(env::var("DATABASE_URL_TOKIDM").expect("DATABASE_URL_TOKIDM not set in .env file")) {
        eprintln!("[ERROR] DATABASE_URL_TOKIDM set(): {:?}", e);
        return;
    }

    let intents = 
          GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MEMBERS
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_MESSAGE_REACTIONS
        | GatewayIntents::DIRECT_MESSAGE_REACTIONS
        | GatewayIntents::GUILD_MESSAGE_TYPING
        | GatewayIntents::DIRECT_MESSAGE_TYPING
        /*
        | GatewayIntents::GUILD_MESSAGE_POLLS
        | GatewayIntents::DIRECT_MESSAGE_POLLS
        */
        ;

    let pool = get_pool()
        .await
        .expect("[ERROR] Database connect()");

    let initial_resp_vec = RespStore::load_all(&pool)
        .await
        .unwrap_or_else(|e| {
            eprintln!("[ERROR] load_all_resp(): {:?}", e);
            Vec::new()
        });

    let resp_store = Arc::new(RespStore::new(initial_resp_vec));

    let blacklist_guild_raw = sqlx::query!(
        r#"
        SELECT id FROM message_blacklist_guild
        "#
    )
    .fetch_all(pool)
    .await
    .unwrap_or_else(|e| {
        eprintln!("[ERROR] load_all message_blacklist_guild: {:?}", e);
        Vec::new()
    });

    let blacklist_guilds = Arc::new(Mutex::new(blacklist_guild_raw.iter().map(|row| row.id).collect::<Vec<u64>>()));
    
    let dm_channels: Arc<Mutex<Vec<(u64, u64)>>> = {
        match get_pool_tokidm().await {
            Err(e) => {
                eprintln!("[ERROR] Database connect() DM: {:?}", e);
                Arc::new(Mutex::new(Vec::new()))
            },
            Ok(pool_dm) => {
                let rows = sqlx::query("SHOW TABLES")
                    .fetch_all(pool_dm)
                    .await
                    .unwrap_or_default();

                Arc::new(Mutex::new(
                    rows.iter()
                        .filter_map(|r| {
                            let name: String = r.try_get(0).ok()?;

                            name.strip_prefix("c")
                                .and_then(|s| s.split_once('u'))
                                .and_then(|(bot, user)| {
                                    Some((
                                        bot.parse::<u64>().ok()?,
                                        user.parse::<u64>().ok()?
                                    ))
                                })
                        })
                        .collect()
                ))
            }
        }
    };

    let trap_ids = {
        let rows = sqlx::query!(
                r#"
                SELECT guild_id, channel_id, log_id FROM trap
                "#
            )
            .fetch_all(pool)
            .await
            .unwrap_or_default();

        Arc::new(Mutex::new(
            rows.iter()
                .map(|r| (r.guild_id, (r.channel_id, r.log_id)))
                .collect()
        ))
    };

    let mut client = Client::builder(TOKEN.get().unwrap(), intents)
        .event_handler(Handler {
            is_ready: Arc::new(AtomicBool::new(false)),
            delete_messages: Arc::new(Mutex::new(Vec::new())),
            stop_flag: Arc::new(AtomicBool::new(false)),
            stop_notify: Arc::new(Notify::new()),
            resp_store: resp_store.clone(),
            resp: resp_store.shared(),
            blacklist_guilds: blacklist_guilds,
            dm_channels: dm_channels,
            trap_ids: trap_ids,
        })
        .await
        .expect("[ERROR] Client builder()");

    if let Err(e) = client.start().await {
        eprintln!("[ERROR] Client start(): {e:?}");
    }
}
