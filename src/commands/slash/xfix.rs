use serenity::all::{
    Channel, ChannelId, ChannelType, CommandInteraction, CreateWebhook, ExecuteWebhook, GuildId,
    Message, Permissions,
};
use serenity::prelude::Context;
use sqlx::MySqlPool;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::respond_ephemeral_embed;
use crate::constants::BOT_USERID;
use crate::utils::lng_trs;

pub type XfixStore = Arc<Mutex<HashSet<u64>>>;
pub const XFIX_WEBHOOK_NAME: &str = "rustoki_xfix";

/// x.com 링크를 fxtwitter.com으로 바꾼다. 바뀐 게 없으면 None.
/// - `https://x.com/...`, `http://x.com/...`, `www.` 포함형 모두 처리
/// - `<...>` 로 감싼(임베드 억제) 링크는 건드리지 않는다
/// - `x.com.evil.com` 같은 더 긴 호스트에는 매칭되지 않는다
pub fn fix_x_links(content: &str) -> Option<String> {
    const SCHEMES: [(&str, &str); 4] = [
        ("https://www.x.com", "https://fxtwitter.com"),
        ("http://www.x.com", "http://fxtwitter.com"),
        ("https://x.com", "https://fxtwitter.com"),
        ("http://x.com", "http://fxtwitter.com"),
    ];

    if !content.contains("x.com") {
        return None;
    }

    let mut out = content.to_string();
    let mut changed = false;

    for (scheme, replacement) in SCHEMES {
        let mut start = 0;
        while let Some(rel) = out[start..].find(scheme) {
            let abs_pos = start + rel;
            let after = abs_pos + scheme.len();
            // `<https://x.com/...>` (임베드 억제)는 스킵
            let wrapped = abs_pos > 0 && out.as_bytes()[abs_pos - 1] == b'<';
            // 베어 도메인(https://x.com 단독)은 스킵: 경로(/...)가 있을 때만 변환
            let rest = &out[after..];
            let has_path = rest.len() > 1
                && rest.starts_with('/')
                && !rest[1..].chars().next().is_none_or(|c| {
                    c.is_whitespace() || matches!(c, '/' | '<' | '>')
                });
            if !wrapped && has_path {
                out.replace_range(abs_pos..after, replacement);
                start = abs_pos + replacement.len();
                changed = true;
            } else {
                start = after;
            }
        }
    }

    if changed { Some(out) } else { None }
}

async fn bot_channel_perms(
    ctx: &Context,
    guild_id: GuildId,
    channel_id: ChannelId,
) -> Option<Permissions> {
    if let Some(guild) = ctx.cache.guild(guild_id) {
        if let (Some(member), Some(channel)) = (
            guild.members.get(&BOT_USERID),
            guild.channels.get(&channel_id),
        ) {
            return Some(guild.user_permissions_in(channel, member));
        }
    }
    let guild = ctx.http.get_guild(guild_id).await.ok()?;
    let bot = ctx.http.get_member(guild_id, BOT_USERID).await.ok()?;
    match ctx.http.get_channel(channel_id).await.ok()? {
        Channel::Guild(ch) => Some(guild.user_permissions_in(&ch, &bot)),
        _ => None,
    }
}

fn is_admin(command: &CommandInteraction) -> bool {
    command.member.as_ref().is_some_and(|m| {
        m.permissions
            .unwrap_or(Permissions::empty())
            .administrator()
    })
}

pub async fn handle_xfix(
    ctx: &Context,
    command: &CommandInteraction,
    xfix: &XfixStore,
    pool: &MySqlPool,
    lng: bool,
) {
    let Some(guild_id) = command.guild_id else {
        respond_ephemeral_embed(ctx, command, lng_trs(lng, "There isn't a guild", "여긴 서버가 아니야"))
            .await;
        return;
    };

    if !is_admin(command) {
        respond_ephemeral_embed(
            ctx,
            command,
            lng_trs(lng, "You are not an admin.", "넌 관리자가 아니야"),
        )
        .await;
        return;
    }

    let guild_id_raw = guild_id.get();
    if xfix.lock().await.contains(&guild_id_raw) {
        respond_ephemeral_embed(
            ctx,
            command,
            lng_trs(lng, "xfix is already enabled", "xfix가 이미 켜져있어"),
        )
        .await;
        return;
    }

    match sqlx::query("INSERT INTO xfix_guild (guild_id) VALUES (?)")
        .bind(guild_id_raw)
        .execute(pool)
        .await
    {
        Ok(_) => {
            xfix.lock().await.insert(guild_id_raw);
            respond_ephemeral_embed(
                ctx,
                command,
                lng_trs(
                    lng,
                    "xfix enabled: x.com links will be reposted as fxtwitter",
                    "xfix를 켰어: x.com 링크는 fxtwitter로 바꿔서 다시 올릴게",
                ),
            )
            .await;
        }
        Err(e) => {
            eprintln!("[ERROR] Command xfix insert: {:?}", e);
            respond_ephemeral_embed(
                ctx,
                command,
                lng_trs(
                    lng,
                    "An error occurred while saving to the database",
                    "DB에 저장하던 중에 오류가 발생했어",
                ),
            )
            .await;
        }
    }
}

pub async fn handle_xfix_del(
    ctx: &Context,
    command: &CommandInteraction,
    xfix: &XfixStore,
    pool: &MySqlPool,
    lng: bool,
) {
    let Some(guild_id) = command.guild_id else {
        respond_ephemeral_embed(ctx, command, lng_trs(lng, "There isn't a guild", "여긴 서버가 아니야"))
            .await;
        return;
    };

    if !is_admin(command) {
        respond_ephemeral_embed(
            ctx,
            command,
            lng_trs(lng, "You are not an admin.", "넌 관리자가 아니야"),
        )
        .await;
        return;
    }

    let guild_id_raw = guild_id.get();
    if !xfix.lock().await.contains(&guild_id_raw) {
        respond_ephemeral_embed(
            ctx,
            command,
            lng_trs(lng, "xfix is not enabled", "xfix가 켜져있지 않아"),
        )
        .await;
        return;
    }

    match sqlx::query("DELETE FROM xfix_guild WHERE guild_id = ?")
        .bind(guild_id_raw)
        .execute(pool)
        .await
    {
        Ok(_) => {
            xfix.lock().await.remove(&guild_id_raw);
            respond_ephemeral_embed(
                ctx,
                command,
                lng_trs(lng, "xfix has been deleted", "xfix를 껐어"),
            )
            .await;
        }
        Err(e) => {
            eprintln!("[ERROR] Command xfix delete: {:?}", e);
            respond_ephemeral_embed(
                ctx,
                command,
                lng_trs(
                    lng,
                    "An error occurred while deleting from the DB",
                    "DB에서 삭제하던 중에 오류가 발생했어",
                ),
            )
            .await;
        }
    }
}

/// 길드에 xfix가 켜져 있으면 x.com 링크 메시지를 웹후크로 다시 올린다.
pub async fn handle_x_message(ctx: &Context, msg: &Message, xfix: &XfixStore) {
    if msg.webhook_id.is_some() {
        return;
    }
    let Some(guild_id) = msg.guild_id else {
        return;
    };
    if !xfix.lock().await.contains(&guild_id.get()) {
        return;
    }
    let Some(fixed) = fix_x_links(&msg.content) else {
        return;
    };
    if fixed.chars().count() > 2000 {
        return;
    }
    if !msg.attachments.is_empty() || !msg.sticker_items.is_empty() {
        eprintln!("[XFIX] skip (has attachments/stickers): {}", msg.id.get());
        return;
    }

    let (display_name, avatar_url) = match ctx.http.get_member(guild_id, msg.author.id).await {
        Ok(member) => (member.display_name().to_string(), member.face()),
        Err(e) => {
            eprintln!("[ERROR] XFIX get_member(): {:?}", e);
            (msg.author.name.clone(), msg.author.face())
        }
    };

    match bot_channel_perms(ctx, guild_id, msg.channel_id).await {
        Some(perms) if perms.manage_messages() && perms.manage_webhooks() => {}
        Some(perms) => {
            eprintln!(
                "[XFIX] missing perms in {} (manage_messages={}, manage_webhooks={})",
                msg.channel_id.get(),
                perms.manage_messages(),
                perms.manage_webhooks()
            );
            return;
        }
        None => {
            eprintln!("[XFIX] cannot determine bot perms, skip");
            return;
        }
    }

    if let Err(e) = ctx
        .http
        .get_channel(msg.channel_id)
        .await
        .map_err(|e| format!("{e:?}"))
        .and_then(|ch| match ch {
            Channel::Guild(ch) if ch.kind == ChannelType::Text => Ok(()),
            _ => Err("not a guild text channel".to_string()),
        })
    {
        eprintln!("[XFIX] channel check: {}", e);
        return;
    }

    let webhook = match msg.channel_id.webhooks(&ctx.http).await {
        Ok(hooks) => hooks
            .into_iter()
            .find(|w| w.name.as_deref() == Some(XFIX_WEBHOOK_NAME)),
        Err(e) => {
            eprintln!("[ERROR] XFIX list webhooks: {:?}", e);
            return;
        }
    };
    let webhook = match webhook {
        Some(w) => w,
        None => {
            match msg
                .channel_id
                .create_webhook(&ctx.http, CreateWebhook::new(XFIX_WEBHOOK_NAME))
                .await
            {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("[ERROR] XFIX create webhook: {:?}", e);
                    return;
                }
            }
        }
    };

    if let Err(e) = webhook
        .execute(
            &ctx.http,
            false,
            ExecuteWebhook::new()
                .content(fixed)
                .username(display_name)
                .avatar_url(avatar_url),
        )
        .await
    {
        eprintln!("[ERROR] XFIX webhook execute(): {:?}", e);
        return;
    }
    if let Err(e) = msg.delete(&ctx.http).await {
        eprintln!("[ERROR] XFIX delete(): {:?}", e);
    }
}
