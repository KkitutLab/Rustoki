use serenity::all::{
    ActivityData, Command, CommandOptionType, CommandType, CreateCommand, CreateCommandOption,
    CreateEmbed, CreateEmbedFooter, CreateMessage, InstallationContext, OnlineStatus, Ready,
};
use serenity::prelude::Context;
use sqlx::MySqlPool;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;

use crate::constants::{BOT_USERID, CHANNELID_LOG_SECRET, GUILD_ID};
use crate::tasks::{spawn_birthday_task, spawn_rainbow_task, spawn_role_gc_task};

pub async fn handle_ready(
    ctx: &Context,
    ready: Ready,
    is_ready_flag: &Arc<AtomicBool>,
    stop_flag: &Arc<AtomicBool>,
    stop_notify: &Arc<Notify>,
    pool: &MySqlPool,
) {
    let is_ready = is_ready_flag.load(Ordering::Relaxed);

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
        println!("Current User is not {}...", BOT_USERID.get());
        return;
    }

    if !is_ready {
        match GUILD_ID
            .set_commands(
                &ctx.http,
                vec![
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
                            CreateCommandOption::new(
                                CommandOptionType::Integer,
                                "birthday",
                                "MMDD",
                            )
                            .required(true)
                            .set_autocomplete(false),
                        ),
                    CreateCommand::new("birthday_remove")
                        .description("Remove your registered birthday")
                        .description_localized("ko", "등록된 생일을 제거"),
                ],
            )
            .await
        {
            Ok(_) => {}
            Err(e) => {
                eprintln!("[ERROR] Commands register(): {:?}", e);
            }
        }

        match Command::set_global_commands(
            &ctx.http,
            vec![
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
                        CreateCommandOption::new(
                            CommandOptionType::Boolean,
                            "server",
                            "Extracts images to server profile if exists",
                        )
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
                        CreateCommandOption::new(
                            CommandOptionType::Boolean,
                            "enabled",
                            "Allow or Deny",
                        )
                        .description_localized("ko", "허용 / 비허용")
                        .required(true)
                        .set_autocomplete(false),
                    )
                    .integration_types(vec![InstallationContext::Guild]),
                CreateCommand::new("call")
                    .description("Talk to the bot")
                    .description_localized("ko", "봇과 대화")
                    .add_option(
                        CreateCommandOption::new(
                            CommandOptionType::String,
                            "string",
                            "Your question",
                        )
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
                        CreateCommandOption::new(
                            CommandOptionType::Channel,
                            "channel",
                            "Trigger Channel",
                        )
                        .description_localized("ko", "발동 채널")
                        .required(true)
                        .set_autocomplete(false),
                    )
                    .add_option(
                        CreateCommandOption::new(CommandOptionType::Channel, "log", "Log Channel")
                            .description_localized("ko", "로그 채널")
                            .required(false)
                            .set_autocomplete(false),
                    )
                    .integration_types(vec![InstallationContext::Guild]),
                CreateCommand::new("z_trap_remove")
                    .description("Remove a trap")
                    .description_localized("ko", "함정을 제거")
                    .integration_types(vec![InstallationContext::Guild]),
                CreateCommand::new("match")
                    .description("Displays the matching virtual compatibility between two users")
                    .description_localized("ko", "두 유저간 가상의 매칭 수치를 출력합니다")
                    .add_option(
                        CreateCommandOption::new(CommandOptionType::User, "user1", "첫 번째 유저")
                            .description_localized("ko", "첫 번째 유저")
                            .required(true),
                    )
                    .add_option(
                        CreateCommandOption::new(CommandOptionType::User, "user2", "두 번째 유저")
                            .description_localized("ko", "두 번째 유저")
                            .required(true),
                    )
                    .integration_types(vec![InstallationContext::Guild, InstallationContext::User]),
                CreateCommand::new("xfix")
                    .description("Auto-fix x.com links as fxtwitter in this server")
                    .description_localized("ko", "이 서버에서 x.com 링크를 fxtwitter로 자동 변환")
                    .integration_types(vec![InstallationContext::Guild]),
                CreateCommand::new("xfix_del")
                    .description("Disable x.com auto-fix in this server")
                    .description_localized("ko", "이 서버에서 x.com 자동 변환 끄기")
                    .integration_types(vec![InstallationContext::Guild]),
            ],
        )
        .await
        {
            Ok(_) => {}
            Err(e) => {
                eprintln!("[ERROR] Global Commands register(): {:?}", e);
            }
        }
    }

    let version_line = format!(
        "v{} ({})",
        env!("CARGO_PKG_VERSION"),
        env!("GIT_DESCRIBE")
    );
    println!("Rustoki {version_line}");

    let embed_on;
    if is_ready {
        #[cfg(debug_assertions)]
        {
            embed_on = CreateEmbed::new()
                .title("DEBUG RECONNECTED")
                .footer(CreateEmbedFooter::new(version_line))
                .color(0x888888);
        }
        #[cfg(not(debug_assertions))]
        {
            embed_on = CreateEmbed::new()
                .title("RECONNECTED")
                .footer(CreateEmbedFooter::new(version_line))
                .color(0x0000FF);
        }
    } else {
        #[cfg(debug_assertions)]
        {
            embed_on = CreateEmbed::new()
                .title("DEBUG")
                .footer(CreateEmbedFooter::new(version_line))
                .color(0x888888);
        }
        #[cfg(not(debug_assertions))]
        {
            embed_on = CreateEmbed::new()
                .title("ON")
                .footer(CreateEmbedFooter::new(version_line))
                .color(0x0000FF);
        }
    }

    let builderon = CreateMessage::new().embed(embed_on);
    if let Err(e) = CHANNELID_LOG_SECRET
        .send_message(&ctx.http, builderon)
        .await
    {
        eprintln!("[ERROR] ON send_message(): {:?}", e);
    }

    ctx.set_presence(
        Some(ActivityData::custom("노닥거리는 중")),
        OnlineStatus::Online,
    );

    if !is_ready {
        is_ready_flag.store(true, Ordering::Relaxed);

        spawn_birthday_task(
            pool.clone(),
            ctx.http.clone(),
            stop_flag.clone(),
            stop_notify.clone(),
        );

        spawn_rainbow_task(ctx.http.clone(), stop_flag.clone(), stop_notify.clone());

        spawn_role_gc_task(ctx.http.clone(), stop_flag.clone(), stop_notify.clone());
    }
}
