use chrono::{DateTime, Utc};
use serenity::all::{
    ChannelId, CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter, CreateMessage, Message,
};
use serenity::prelude::Context;

use crate::constants::RTCMD_USAGE_EMBED;
use crate::utils::parse_quoted;

pub async fn handle_embed(ctx: &Context, msg: &Message, rest: &str) {
    let arg = rest.trim();
    if arg.is_empty() {
        if let Err(e) = msg.channel_id.say(&ctx.http, RTCMD_USAGE_EMBED).await {
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
            let _ = msg
                .reply_mention(&ctx.http, "ChannelId must be a u64 integer")
                .await;
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
                        match parse_quoted(
                            &mut std::iter::once(first).chain(&mut iter),
                            "--authortext",
                            256,
                        ) {
                            Ok(t) => author_text = Some(t),
                            Err(e) => error_logs.push(e),
                        }
                    }
                } else {
                    error_logs.push(String::from("--authortext null"));
                }
            }
            "--authorurl" | "-au" => {
                if let Some(u) = iter.next() {
                    author_url = Some(String::from(u));
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
            "--description" | "-d" => match parse_quoted(&mut iter, "--description", 4096) {
                Ok(t) => description = Some(t),
                Err(e) => {
                    error_logs.push(e);
                }
            },
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
                        match parse_quoted(
                            &mut std::iter::once(first).chain(&mut iter),
                            "--footertext",
                            2048,
                        ) {
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
                if let Some(u) = iter.next() {
                    image = Some(u);
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
            "--title" | "-t" => match parse_quoted(&mut iter, "--title", 256) {
                Ok(t) => title = Some(t),
                Err(e) => {
                    error_logs.push(e);
                }
            },
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
        let mut reply_text = format!(
            "```\n!! {} error{} occurred:\n",
            count,
            if count > 1 { "s" } else { "" }
        );
        reply_text.push_str(&error_logs.join("\n"));
        reply_text.push_str("\n```");
        let _ = msg.reply_mention(&ctx.http, reply_text).await;
        return;
    }

    let mut embed = CreateEmbed::new();

    if let Some(text) = author_text {
        let mut author = CreateEmbedAuthor::new(text);

        if let Some(u) = author_url {
            author = author.url(u);
        }

        if let Some(avatar) = author_avatar {
            author = author.icon_url(avatar);
        }
        embed = embed.author(author);
    } else if author_url.is_some() || author_avatar.is_some() {
        error_logs.push(String::from(
            "!! --authorurl/--authoravatar requires --authortext",
        ));
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
        let _ = msg
            .reply_mention(&ctx.http, format!("```\n!! Message send FAIL:\n{:?}```", e))
            .await;
    }
}
