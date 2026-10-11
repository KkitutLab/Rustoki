pub mod db;
pub mod regions;
pub mod special;
pub mod weather;

pub use db::{
    db_add_key, db_add_key_bulk, db_add_resp, db_add_resp_bulk, db_get_table, db_new_key,
    db_remove_resp, db_remove_table, Resp, RespStore,
};
pub use special::get_special_response;

use chrono::{Local, Utc};
use meval::eval_str;
use rand::RngExt;
use std::sync::Arc;
use tokio::sync::Mutex;

const TIME_KEYWORDS: &[&str] = &[
    "몇시야",
    "시간",
    "지금몇시야",
    "시간좀",
    "몇시",
    "날짜",
    "몇일",
    "WHATSTIME",
    "TIME",
    "WHATTIMEISIT",
    "GIVEMETIME",
    "DATE",
    "WHATS",
    "WHATSDATE",
    "KST",
    "KOREANSTANDARDTIME",
    "KOREASTANDARDTIME",
];

const UTC_KEYWORDS: &[&str] = &[
    "UTC",
    "협정시간",
    "협정세계시",
    "세계협정시계시",
    "COORDINATEDUNIVERSALTIME",
    "UNIVERSALTIMECOORDINATED",
];

pub async fn get_response(resp: &Arc<Mutex<Vec<Resp>>>, message: &str) -> Option<String> {
    /* ㅋ
    if message == "?" {
        if rand::rng().random_range(0..1000) == 1 {
            return Some("러스트에서 `?` 연산자는 주로 에러 처리를 간단하게 하고자 할 때 사용되며 특정 연산이 실패할 가능성이 있는 상황에서 Result 또는 Option 타입을 다루는 데 유용하지.\n`?`를 이용하면 함수 내부에서 발생한 에러를 간결하게 호출한 쪽으로 전달할 수 있어.\n좋아, 말이 길었던것 같아. 뭐 어쨌든 너가 극한의 확률을 뚫었어.\n축하해줄게 이건 나오기 정말 정말 어려워. 근데 넌 그걸 뚫은거고".to_string())
        } else {
            return None;
        }
    }
    */

    let normalized_q = message.replace(&[' ', '.', '!', ':', '-'][..], "");

    {
        let guard = resp.lock().await;

        if guard.is_empty() {
            return Some("DB가 비어있는데, 지금은 쓸 수 없을 것 같아.".to_string());
        }

        for r in guard.iter() {
            if r.values.iter().any(|kw| {
                kw.replace(&[' ', '.'][..], "")
                    .eq_ignore_ascii_case(&normalized_q)
            }) {
                let idx = if r.probs.len() > 1 {
                    rand::rng().random_range(0..r.probs.len())
                } else {
                    0
                };
                return Some(r.probs[idx].clone());
            }
        }
    }

    let normalized = message.replace(&[' ', '?', '!', '.', ':', '-'][..], "");

    for &keyword in UTC_KEYWORDS {
        if normalized == keyword {
            let utc = Utc::now().format("%Y-%m-%d %H:%M:%S");
            return Some(format!("UTC {}", utc));
        }
    }

    for &keyword in TIME_KEYWORDS {
        if normalized == keyword {
            let kst = Local::now().format("%Y-%m-%d %H:%M:%S");
            return Some(format!("KST {}", kst));
        }
    }

    {
        let guard = resp.lock().await;

        for r in guard.iter() {
            if r.values.iter().any(|kw| {
                kw.replace(&[' ', '?', '!', '.', ':', '-'][..], "")
                    .eq_ignore_ascii_case(&normalized)
            }) {
                let idx = if r.probs.len() > 1 {
                    rand::rng().random_range(0..r.probs.len())
                } else {
                    0
                };
                return Some(r.probs[idx].clone());
            }
        }
    }

    let evalmsg = message.replace(' ', "").to_lowercase();
    if let Ok(result) = eval_str(&evalmsg) {
        return Some(result.to_string());
    }

    None
}
