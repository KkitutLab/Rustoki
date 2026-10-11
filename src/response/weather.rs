use chrono::{Local, Timelike};
use once_cell::sync::Lazy;
use reqwest::get;
use serde_json::Value;
use serenity::all::CreateEmbed;
use std::env;

use super::special::SpecialMessage;

static WEATHER_KEY: Lazy<String> =
    Lazy::new(|| env::var("WEATHER_KEY").expect("[EXPECT] env WEATHER_KEY"));

pub async fn fetch_weather(name: &str, nx: u8, ny: u8) -> Option<SpecialMessage> {
    let now = Local::now();
    let base_date = now.format("%Y%m%d").to_string();
    let base_time = format!("{:02}00", now.hour());

    let url = format!(
        "http://apis.data.go.kr/1360000/VilageFcstInfoService_2.0/getUltraSrtNcst?serviceKey={}\
        &pageNo=1&numOfRows=100&dataType=JSON\
        &base_date={}&base_time={}\
        &nx={}&ny={}",
        *WEATHER_KEY, base_date, base_time, nx, ny
    );

    match get(&url).await {
        Ok(res) => match res.json::<Value>().await {
            Ok(weather_data) => {
                let mut pty = None;
                let mut reh = None;
                let mut rn1 = None;
                let mut t1h = None;
                let mut vec = None;
                let mut wsd = None;

                if let Some(items) = weather_data["response"]["body"]["items"]["item"].as_array() {
                    for item in items {
                        if let (Some(category), Some(obsr_value)) =
                            (item["category"].as_str(), item["obsrValue"].as_str())
                        {
                            match category {
                                "PTY" => pty = Some(obsr_value),
                                "REH" => reh = Some(obsr_value),
                                "RN1" => rn1 = Some(obsr_value),
                                "T1H" => t1h = Some(obsr_value),
                                "VEC" => vec = obsr_value.parse::<i32>().ok(),
                                "WSD" => wsd = Some(obsr_value),
                                _ => {}
                            }
                        }
                    }
                }

                let mut embed = CreateEmbed::new();
                let mut weather_info = String::new();

                if let Some(pty) = pty {
                    let (desc, icon, col) = match pty {
                        "0" => ("맑음", "☀️", 0x87CEEB),             // SkyBlue
                        "1" => ("비", "🌧️", 0x4682B4),               // SteelBlue
                        "2" => ("비/눈", "🌧️/❄️", 0x6A5ACD),         // SlateBlue
                        "3" => ("눈", "❄️", 0xADD8E6),               // LightBlue
                        "5" => ("빗방울", "🌦️", 0x5F9EA0),           // CadetBlue
                        "6" => ("빗방울/눈날림", "🌦️/❄️", 0x7B68EE), // MediumSlateBlue
                        "7" => ("눈날림", "❄️", 0xB0E0E6),           // PowderBlue
                        _ => ("알수없음", "❓", 0xAAAAAA),
                    };
                    weather_info.push_str(&format!("> # {}\n## {}\n", icon, desc));
                    embed = embed.color(col);
                }
                if let Some(v) = t1h {
                    weather_info.push_str(&format!("기온 {}°C ", v));
                }
                if let Some(v) = reh {
                    weather_info.push_str(&format!("습도 {}%\n", v));
                }
                if let Some(deg) = vec {
                    let arrow = match deg {
                        338..=359 | 0..=22 => "⬆️",
                        23..=67 => "↗️",
                        68..=112 => "➡️",
                        113..=157 => "↘️",
                        158..=202 => "⬇️",
                        203..=247 => "↙️",
                        248..=292 => "⬅️",
                        293..=337 => "↖️",
                        _ => "❓",
                    };
                    weather_info.push_str(&format!("**바람**: {} {}°\n", arrow, deg));
                }
                if let Some(v) = wsd {
                    weather_info.push_str(&format!("**풍속**: {} m/s\n", v));
                }
                if let Some(v) = rn1 {
                    weather_info.push_str(&format!("**최근 강수량**: {}mm\n", v));
                }
                if !weather_info.is_empty() {
                    embed = embed
                        .title(format!("{} >>", name))
                        .description(weather_info);
                    return Some(
                        SpecialMessage::new()
                            .embed(embed)
                            .extra_note("*weather_info_now*"),
                    );
                }

                Some(
                    SpecialMessage::new()
                        .content("정보가 없어. 오류일 가능성이 높아")
                        .content_to_extra(),
                )
            }
            Err(e) => {
                println!("[ERROR] Weather API JSON: {:?}", e);
                Some(
                    SpecialMessage::new()
                        .content("데이터 처리 중에 오류가 났어")
                        .content_to_extra(),
                )
            }
        },
        Err(e) => {
            println!("[ERROR] Weather API JSON: {:?}", e);
            Some(
                SpecialMessage::new()
                    .content("API 요청 중에 오류가 났어")
                    .content_to_extra(),
            )
        }
    }
}
