use serenity::all::{Color, EditRole, RoleId};
use serenity::http::Http;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::time::{sleep, Duration};

use crate::constants::GUILD_ID;

pub fn spawn_rainbow_task(http: Arc<Http>, stop_flag: Arc<AtomicBool>, stop_notify: Arc<Notify>) {
    let rainbow_id = RoleId::new(1363166115165110513);
    tokio::spawn(async move {
        let colors: [u32; 12] = [
            0xFF0000, 0xFF8000, 0xFFFF00, 0x80FF00, 0x00FF00, 0x00FF80, 0x00FFFF, 0x0080FF,
            0x0000FF, 0x8000FF, 0xFF00FF, 0xFF0080,
        ];

        let mut index = 0;

        loop {
            let rolebuilder = EditRole::new().colour(Color::from(colors[index]));
            if let Err(e) = GUILD_ID.edit_role(&http, rainbow_id, rolebuilder).await {
                eprintln!("[ERROR] GUILD_ID edit_role color: {:?}", e);
            }

            index = (index + 1) % colors.len();

            tokio::select! {
                _ = sleep(Duration::from_secs(240)) => {}
                _ = stop_notify.notified() => {
                    if stop_flag.load(Ordering::Relaxed) {
                        println!("[SYS] Rainbow task Shutdown");
                        break;
                    }
                }
            }
        }
    });
}
