use serenity::all::{
    CreateAttachment, CreateEmbed, CreateInteractionResponseMessage, CreateMessage,
};
use serenity::prelude::Context;

use super::regions::{W_WEATHER_KEYWORDS, W_WEATHER_REGIONS};
use super::weather::fetch_weather;

const W_FACE: &[&str] = &["니얼굴", "URFACE"];
const W_MIDDLE: &[&str] = &["ㅗ", "MIDDLEFINGER"];
const W_BLUEPRINT: &[&str] = &["블루프린트", "청사진", "BLUEPRINT"];

pub struct SpecialMessage {
    pub content: Option<String>,
    pub sticker_id: Option<u64>,
    pub embed: Option<CreateEmbed>,
    pub attachment: Option<CreateAttachment>,
    pub extra_note: String,
}

impl SpecialMessage {
    pub fn new() -> Self {
        Self {
            content: None,
            sticker_id: None,
            embed: None,
            attachment: None,
            extra_note: String::new(),
        }
    }

    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    pub fn sticker_id(mut self, id: u64) -> Self {
        self.sticker_id = Some(id);
        self
    }

    pub fn embed(mut self, embed: CreateEmbed) -> Self {
        self.embed = Some(embed);
        self
    }

    pub fn attachment(mut self, attachment: CreateAttachment) -> Self {
        self.attachment = Some(attachment);
        self
    }

    pub fn extra_note(mut self, note: impl Into<String>) -> Self {
        self.extra_note = note.into();
        self
    }

    pub fn content_to_extra(mut self) -> Self {
        if let Some(ref c) = self.content {
            self.extra_note = c.clone();
        }
        self
    }

    pub fn content_add_extra(mut self, extra: impl Into<String>) -> Self {
        if let Some(ref c) = self.content {
            self.extra_note = format!("{}{}", self.extra_note, c);
        }
        self.extra_note.push_str(&extra.into());
        self
    }

    pub fn to_create_message(&self) -> CreateMessage {
        let mut msg = CreateMessage::new();

        if let Some(content) = &self.content {
            msg = msg.content(content);
        }
        if let Some(sticker_id) = self.sticker_id {
            msg = msg.sticker_id(sticker_id);
        }
        if let Some(embed) = &self.embed {
            msg = msg.embed(embed.clone());
        }
        if let Some(attachment) = &self.attachment {
            msg = msg.add_file(attachment.clone());
        }

        msg
    }

    pub fn to_interaction_response(&self) -> CreateInteractionResponseMessage {
        let mut msg = CreateInteractionResponseMessage::new();

        if self.embed.is_some() {
            if let Some(embed) = &self.embed {
                msg = msg.embed(embed.clone());
            }
            if let Some(content) = &self.content {
                msg = msg.content(content);
            }
        } else if let Some(content) = &self.content {
            let embed = CreateEmbed::new().description(content).color(0xFFAAAA);
            msg = msg.embed(embed);
        }

        if let Some(attachment) = &self.attachment {
            msg.add_file(attachment.clone())
        } else {
            msg
        }
    }
}

pub async fn get_special_response(message: &str, ctx: &Context) -> Option<SpecialMessage> {
    let normalized_message = message.replace(&[' ', '?', '!', '.', ':', '-'][..], "");

    match normalized_message.as_str() {
        msg if W_FACE.iter().any(|&word| msg.contains(word)) => Some(
            SpecialMessage::new()
                .sticker_id(1296116499790168065)
                .extra_note("*sticker_1296116499790168065*"),
        ),
        msg if W_MIDDLE.iter().any(|&word| msg.contains(word)) => Some(
            SpecialMessage::new()
                .sticker_id(1296127617438711869)
                .extra_note("*sticker_1296127617438711869*"),
        ),
        msg if W_BLUEPRINT.iter().any(|&word| msg.contains(word)) => {
            let mut spmsg = SpecialMessage::new()
                .content("쉬운 비주얼 스크립팅 언어, Blueprint야. 코드 없이도 게임 로직을 구현할 수 있어")
                .content_add_extra(": *Blueprint_hi*");

            if let Ok(create_attachment) = CreateAttachment::url(
                &ctx.http,
                "https://cdn.discordapp.com/attachments/1270996130615922700/1356206035819302993/Blueprint_hi.png?ex=67ebb8cc&is=67ea674c&hm=df1cf4d441ee6cd9633adc24145e0e7083b52b2114b8187c51668514fd1c556b&",
            )
            .await
            {
                spmsg = spmsg.attachment(create_attachment);
            }

            Some(spmsg)
        }
        msg if W_WEATHER_KEYWORDS.iter().any(|&word| msg.contains(word)) => {
            for (name, &(nx, ny)) in &W_WEATHER_REGIONS {
                if normalized_message.contains(&name.to_uppercase()) {
                    return fetch_weather(name, nx, ny).await;
                }
            }

            Some(
                SpecialMessage::new()
                    .content("지역명이 없어")
                    .content_to_extra(),
            )
        }
        _ => None,
    }
}
