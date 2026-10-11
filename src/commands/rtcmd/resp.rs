use serenity::all::Message;
use serenity::prelude::Context;
use sqlx::MySqlPool;
use std::sync::Arc;

use crate::constants::RTCMD_USAGE_RESP;
use crate::response::{
    db_add_key, db_add_key_bulk, db_add_resp, db_add_resp_bulk, db_get_table, db_new_key,
    db_remove_resp, db_remove_table, RespStore,
};

pub async fn handle_resp(
    ctx: &Context,
    msg: &Message,
    pool: &MySqlPool,
    resp_store: &Arc<RespStore>,
    rtcmd: &str,
) {
    let mut response;
    let arg = rtcmd.trim_start_matches("resp").trim();

    if arg.is_empty() {
        response = RTCMD_USAGE_RESP.to_string();
    } else if arg.starts_with("update") {
        if arg.trim() != "update" {
            response = String::from("[ERR] Invalid arguments for 'update'");
        } else {
            match RespStore::load_all(pool).await {
                Ok(list) => {
                    resp_store.update(list).await;
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
                response = String::from(
                    "[ERR] 'new_key' must be uppercase and contain no spaces (use -a to auto-fix)",
                );
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
        let rest_clean = rest
            .replace("-a", "")
            .replace("--auto", "")
            .trim()
            .to_string();

        if rest_clean.starts_with('[') {
            let mut it = rest_clean.splitn(2, ']').map(str::trim);
            let first_key_part = it.next();
            let keys_json_part = it.next();

            if let (Some(fk_raw), Some(kj_raw)) = (first_key_part, keys_json_part) {
                let mut first_key = fk_raw
                    .trim_start_matches('"')
                    .trim_end_matches('"')
                    .to_string();
                if auto {
                    first_key = first_key.to_uppercase().replace(' ', "");
                }

                if first_key.contains(' ') || first_key != first_key.to_uppercase() {
                    response = String::from(
                        "[ERR] 'add_key' first key must be uppercase and contain no spaces (use -a to auto-fix)",
                    );
                } else {
                    match serde_json::from_str::<Vec<String>>(kj_raw) {
                        Ok(mut second_keys) => {
                            if auto {
                                for k in second_keys.iter_mut() {
                                    *k = k.to_uppercase().replace(' ', "");
                                }
                            }
                            let invalid = second_keys
                                .iter()
                                .any(|k| k.contains(' ') || *k != k.to_uppercase());
                            if invalid {
                                response = String::from(
                                    "[ERR] 'add_key' second keys must be uppercase and contain no spaces (use -a to auto-fix)",
                                );
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
                        Err(e) => {
                            response = format!("[ERR] Invalid JSON format for 'add_key': {:?}", e)
                        }
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
                    first_key = first_key.to_uppercase().replace(' ', "");
                    second_key = second_key.to_uppercase().replace(' ', "");
                }

                if first_key.contains(' ')
                    || first_key != first_key.to_uppercase()
                    || second_key.contains(' ')
                    || second_key != second_key.to_uppercase()
                {
                    response = String::from(
                        "[ERR] 'add_key' keys must be uppercase and contain no spaces (use -a to auto-fix)",
                    );
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
                    Ok(probs) => match db_add_resp_bulk(pool, key, &probs).await {
                        Ok(_) => response = String::from("add (bulk) completed"),
                        Err(e) => {
                            eprintln!("[RESP] db_add_resp_bulk(): {:?}", e);
                            response = format!("[ERR] add (bulk) failed: {:?}", e);
                        }
                    },
                    Err(e) => response = format!("[ERR] Invalid JSON array for 'add': {:?}", e),
                }
            } else if !(value_str.starts_with('"') && value_str.ends_with('"')) {
                response = String::from("[ERR] Value must be quoted for single add");
            } else {
                let value = &value_str[1..value_str.len() - 1];
                match db_add_resp(pool, key, value).await {
                    Ok(_) => response = String::from("add completed"),
                    Err(e) => {
                        eprintln!("[RESP] db_add_resp(): {:?}", e);
                        response = format!("[ERR] add failed: {:?}", e);
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
        } else if !parts.contains(&"DOUBLECHECK".to_string()) {
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
            let target_unquoted = &target_str[1..target_str.len() - 1];

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
