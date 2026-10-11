use serenity::all::{
    CommandInteraction, CreateEmbed, CreateInteractionResponseFollowup, Route, Timestamp, UserId,
};
use serenity::http::LightMethod;
use serenity::http::Request;
use serenity::json::Value;
use serenity::prelude::Context;

use super::respond_ephemeral_embed;
use crate::utils::image::update_avatar_size;
use crate::utils::lng_trs;

async fn fetch_discord_json(
    ctx: &Context,
    route: Route<'_>,
    lng: bool,
    not_found_msg: (&str, &str),
    length_msg: (&str, &str),
) -> String {
    let request = Request::new(route, LightMethod::Get);

    match ctx.http.request(request).await {
        Ok(response) => match response.bytes().await {
            Ok(body_bytes) => match serde_json::from_slice::<Value>(&body_bytes) {
                Ok(json_value) => {
                    let response_json =
                        serde_json::to_string_pretty(&json_value).unwrap_or_default();
                    let mut txt = format!(
                        "```json\n{}```",
                        response_json.replace('`', "\\`").replace('@', "＠")
                    );

                    if txt.len() > 4096 {
                        txt = format!(
                            "{}: ({})",
                            lng_trs(lng, length_msg.0, length_msg.1),
                            txt.len()
                        );
                    }

                    txt
                }
                Err(_) => lng_trs(lng, "Failed to parse JSON", "JSON 파싱에 실패했어").to_string(),
            },
            Err(_) => lng_trs(
                lng,
                "Failed to read response body",
                "응답 본문 읽기에 실패했어",
            )
            .to_string(),
        },
        Err(_) => lng_trs(lng, not_found_msg.0, not_found_msg.1).to_string(),
    }
}

pub async fn handle_spy_data(ctx: &Context, command: &CommandInteraction, lng: bool) {
    let respstr = if let Some(message_id) = command.data.target_id {
        let route = Route::ChannelMessage {
            channel_id: command.channel_id.get().into(),
            message_id: message_id.get().into(),
        };
        fetch_discord_json(
            ctx,
            route,
            lng,
            ("Message not found", "메시지를 찾을 수 없었어"),
            ("Message is too long", "메시지가 너무 길어"),
        )
        .await
    } else {
        lng_trs(
            lng,
            "Unable to get message ID",
            "메시지 ID를 가져올 수 없었어",
        )
        .to_string()
    };

    respond_ephemeral_embed(ctx, command, &respstr).await;
}

pub async fn handle_spy_channel(ctx: &Context, command: &CommandInteraction, lng: bool) {
    let route = Route::Channel {
        channel_id: command.channel_id.get().into(),
    };
    let respstr = fetch_discord_json(
        ctx,
        route,
        lng,
        ("Channel not found", "채널을 찾을 수 없었어"),
        ("Data is too long", "데이터가 너무 길어"),
    )
    .await;

    respond_ephemeral_embed(ctx, command, &respstr).await;
}

pub async fn handle_spy_profile(ctx: &Context, command: &CommandInteraction, lng: bool) {
    if let Err(e) = command.defer(&ctx.http).await {
        eprintln!("[ERROR] Command defer(): {:?}", e);
        return;
    }

    if let Some(opt) = command.data.options.first() {
        if let Some(userid) = opt.value.as_user_id() {
            let user_id = UserId::new(userid.into());

            if let Ok(user) = user_id.to_user(&ctx.http).await {
                let username = user.name.clone();
                let mut embed = CreateEmbed::new()
                    .title(userid.get().to_string())
                    .color(0xFFAAAA)
                    .timestamp(Timestamp::now());
                let global_display_name =
                    user.global_name.clone().unwrap_or_else(|| username.clone());

                if let Some(guild_id) = command.guild_id {
                    if let Ok(member) = guild_id.member(&ctx.http, user_id).await {
                        if let Some(ref servername) = member.nick {
                            embed = embed.description(format!(
                                "**{}:** `{}`\n**{}:** `{}`\n**{}:** `{}`",
                                lng_trs(lng, "Name", "이름"),
                                username,
                                lng_trs(lng, "Global Name", "전역 이름"),
                                global_display_name,
                                lng_trs(lng, "Server Name", "서버 이름"),
                                servername
                            ));
                        } else {
                            embed = embed.description(format!(
                                "**{}:** `{}`\n**{}:** `{}`",
                                lng_trs(lng, "Name", "이름"),
                                username,
                                lng_trs(lng, "Global Name", "전역 이름"),
                                global_display_name
                            ));
                        }
                        let userfaceup = update_avatar_size(&user.face());
                        let thumbnail_url = if let Some(guild_avatar) = member.avatar_url() {
                            if let Some(option) = command.data.options.get(1) {
                                if let Some(isserver) = option.value.as_bool() {
                                    if isserver {
                                        update_avatar_size(&guild_avatar)
                                    } else {
                                        userfaceup
                                    }
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
                        match ctx.http.get_user(user_id).await {
                            Ok(user) => {
                                embed = embed
                                    .thumbnail(update_avatar_size(&user.face()))
                                    .description(format!(
                                        "**{}:** `{}`",
                                        lng_trs(lng, "Name", "이름"),
                                        user.name
                                    ));
                            }
                            Err(e) => {
                                eprintln!("[ERROR] get_user_info: {:?}", e);
                                embed = embed.description(lng_trs(
                                    lng,
                                    "User not found",
                                    "유저를 찾을 수 없었어",
                                ));
                            }
                        }
                    }
                }
                if let Err(e) = command
                    .create_followup(
                        &ctx.http,
                        CreateInteractionResponseFollowup::new().add_embed(embed),
                    )
                    .await
                {
                    eprintln!("[ERROR] spy_profile response: {:?}", e);
                }
            } else if let Err(e) = command
                .create_followup(
                    &ctx.http,
                    CreateInteractionResponseFollowup::new().content(lng_trs(
                        lng,
                        "User not found",
                        "유저를 찾을 수 없었어",
                    )),
                )
                .await
            {
                eprintln!("[ERROR] spy_profile response: {:?}", e);
            }
        } else if let Err(e) = command
            .create_followup(
                &ctx.http,
                CreateInteractionResponseFollowup::new()
                    .content(lng_trs(
                        lng,
                        "User ID field is empty",
                        "유저 ID란이 비어있어",
                    ))
                    .ephemeral(true),
            )
            .await
        {
            eprintln!("[ERROR] Command create_response(): {:?}", e);
        }
    }
}
