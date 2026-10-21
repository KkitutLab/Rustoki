use serenity::all::{CreateEmbed, CreateMessage};
use serenity::http::Http;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::time::{sleep, Duration};

use crate::constants::CHANNELID_LOG_SECRET;

const STATE_FILE: &str = ".deploy-state";

pub fn spawn_deploy_watch_task(
    http: Arc<Http>,
    stop_flag: Arc<AtomicBool>,
    stop_notify: Arc<Notify>,
) {
    tokio::spawn(async move {
        let mut last = read_state();
        loop {
            tokio::select! {
                _ = sleep(Duration::from_secs(3)) => {}
                _ = stop_notify.notified() => {
                    if stop_flag.load(Ordering::Relaxed) {
                        println!("[SYS] DeployWatch task Shutdown");
                        break;
                    }
                }
            }
            let cur = read_state();
            if cur == last {
                continue;
            }
            last = cur.clone();
            let Some((kind, tag)) = cur.as_deref().and_then(parse_state) else {
                continue;
            };
            announce(&http, kind, tag).await;
        }
    });
}

fn read_state() -> Option<String> {
    std::fs::read_to_string(STATE_FILE)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn parse_state(s: &str) -> Option<(&str, &str)> {
    let (kind, tag) = s.split_once(' ')?;
    match kind {
        "updating" | "rebooting" => Some((kind, tag.trim())),
        _ => None,
    }
}

async fn announce(http: &Http, kind: &str, tag: &str) {
    #[cfg(debug_assertions)]
    let title = format!("?{}", kind.to_uppercase());
    #[cfg(not(debug_assertions))]
    let title = kind.to_uppercase();

    #[cfg(debug_assertions)]
    {
        println!("Rustoki ?{kind} -> v{tag}");
    }
    #[cfg(not(debug_assertions))]
    {
        println!("Rustoki !{kind} -> v{tag}");
    }

    let embed = CreateEmbed::new()
        .title(title)
        .description(format!("v{tag}"))
        .color(0xFFFF00);
    let builder = CreateMessage::new().embed(embed);
    if let Err(e) = CHANNELID_LOG_SECRET.send_message(http, builder).await {
        eprintln!("[ERROR] DeployWatch send_message(): {:?}", e);
    }
}
