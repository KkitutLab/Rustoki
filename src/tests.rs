use std::sync::Arc;
use tokio::sync::Mutex;

use crate::commands::slash::match_cmd::calculate_match_score;
use crate::handler::message::extract_call_query;
use crate::response::regions::W_WEATHER_REGIONS;
use crate::response::special::SpecialMessage;
use crate::response::{get_response, Resp};
use crate::upboard::get_pin_icon_url;
use crate::utils::{lng_trs, parse_quoted};

#[test]
fn test_lng_trs() {
    assert_eq!(lng_trs(true, "en", "ko"), "ko");
    assert_eq!(lng_trs(false, "en", "ko"), "en");
    assert_eq!(lng_trs(true, 10, 20), 20);
    assert_eq!(lng_trs(false, 10, 20), 10);
}

#[test]
fn test_parse_quoted_valid_single_word() {
    let input = vec!["\"hello\""];
    let mut iter = input.into_iter();
    let res = parse_quoted(&mut iter, "test", 100);
    assert_eq!(res, Ok("hello".to_string()));
}

#[test]
fn test_parse_quoted_valid_multiple_words() {
    let input = vec!["\"hello", "world", "rustoki\""];
    let mut iter = input.into_iter();
    let res = parse_quoted(&mut iter, "test", 100);
    assert_eq!(res, Ok("hello world rustoki".to_string()));
}

#[test]
fn test_parse_quoted_missing_start_quote() {
    let input = vec!["hello\"", "world\""];
    let mut iter = input.into_iter();
    let res = parse_quoted(&mut iter, "test", 100);
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("must start with a quote"));
}

#[test]
fn test_parse_quoted_missing_closing_quote() {
    let input = vec!["\"hello", "world"];
    let mut iter = input.into_iter();
    let res = parse_quoted(&mut iter, "test", 100);
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("missing closing quote"));
}

#[test]
fn test_parse_quoted_empty_quotes() {
    let input = vec!["\"\""];
    let mut iter = input.into_iter();
    let res = parse_quoted(&mut iter, "test", 100);
    assert_eq!(res, Ok("".to_string()));
}

#[test]
fn test_parse_quoted_max_len_exceeded() {
    let input = vec!["\"123456\""];
    let mut iter = input.into_iter();
    let res = parse_quoted(&mut iter, "test", 5);
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("length must be <="));
}

#[test]
fn test_parse_quoted_empty_input() {
    let input: Vec<&str> = vec![];
    let mut iter = input.into_iter();
    let res = parse_quoted(&mut iter, "test", 100);
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("null"));
}

#[test]
fn test_pin_icon_urls() {
    assert!(get_pin_icon_url(0).ends_with("2b06.png"));
    assert!(get_pin_icon_url(1).ends_with("1f53c.png"));
    assert!(get_pin_icon_url(2).ends_with("23eb.png"));
    assert!(get_pin_icon_url(3).ends_with("1f4c8.png"));
    assert!(get_pin_icon_url(0).starts_with("https:"));
}

#[test]
fn test_calculate_match_score() {
    let s1 = calculate_match_score(123456789, 987654321);
    let s2 = calculate_match_score(987654321, 123456789);
    assert_eq!(s1, s2);
    assert!(s1 <= 100);

    let same_id_score = calculate_match_score(11111, 11111);
    assert!(same_id_score <= 100);

    let boundary_score = calculate_match_score(0, u64::MAX);
    assert!(boundary_score <= 100);
}

#[test]
fn test_extract_call_query() {
    assert_eq!(
        extract_call_query("러스토키 안녕"),
        Some("안녕".to_string())
    );
    assert_eq!(
        extract_call_query("러스토키   날씨 알려줘  "),
        Some("날씨 알려줘".to_string())
    );
    assert_eq!(extract_call_query("<\" ping"), Some("PING".to_string()));
    assert_eq!(extract_call_query("러스토키 "), None);
    assert_eq!(extract_call_query("러스토키"), None);
    assert_eq!(extract_call_query("<\" "), None);
    assert_eq!(extract_call_query("<\""), None);
    assert_eq!(extract_call_query("일반 메시지"), None);
    assert_eq!(extract_call_query(""), None);
}

#[test]
fn test_regions_phf_lookup() {
    assert_eq!(W_WEATHER_REGIONS.get("서울특별시종로구"), Some(&(60, 127)));
    assert_eq!(W_WEATHER_REGIONS.get("서울용산구"), Some(&(60, 126)));
    assert_eq!(W_WEATHER_REGIONS.get("없는동네이름"), None);
}

#[test]
fn test_special_message_builder() {
    let sp = SpecialMessage::new()
        .content("test_content")
        .sticker_id(99999)
        .extra_note("init_note");

    assert_eq!(sp.content, Some("test_content".to_string()));
    assert_eq!(sp.sticker_id, Some(99999));
    assert_eq!(sp.extra_note, "init_note");

    let sp_extra = SpecialMessage::new().content("copy_me").content_to_extra();
    assert_eq!(sp_extra.extra_note, "copy_me");

    let sp_append = SpecialMessage::new()
        .extra_note("pfx_")
        .content("body")
        .content_add_extra("_sfx");
    assert_eq!(sp_append.extra_note, "pfx_body_sfx");
}

#[tokio::test]
async fn test_get_response() {
    let empty_store = Arc::new(Mutex::new(Vec::new()));
    let empty_resp = get_response(&empty_store, "안녕").await;
    assert_eq!(
        empty_resp,
        Some("DB가 비어있는데, 지금은 쓸 수 없을 것 같아.".to_string())
    );

    let store = Arc::new(Mutex::new(vec![Resp {
        values: vec!["안녕".to_string()],
        probs: vec!["반가워".to_string()],
    }]));

    assert_eq!(
        get_response(&store, "안녕").await,
        Some("반가워".to_string())
    );
    assert_eq!(
        get_response(&store, "1 + 2 * 3").await,
        Some("7".to_string())
    );

    let utc_res = get_response(&store, "UTC").await;
    assert!(utc_res.is_some());
    assert!(utc_res.unwrap().starts_with("UTC "));

    let kst_res = get_response(&store, "몇시야").await;
    assert!(kst_res.is_some());
    assert!(kst_res.unwrap().starts_with("KST "));

    let none_res = get_response(&store, "알수없는메시지XYZ").await;
    assert_eq!(none_res, None);
}

#[tokio::test]
async fn test_system_memory() {
    let mem = crate::utils::system::get_vmrss_mb().await;
    assert!(mem.is_err() || mem.unwrap() >= 0.0);
}

#[test]
fn test_parse_quoted_boundary_length() {
    let input_exact = vec!["\"12345\""];
    let mut iter_exact = input_exact.into_iter();
    assert_eq!(
        parse_quoted(&mut iter_exact, "test", 5),
        Ok("12345".to_string())
    );

    let input_over = vec!["\"123456\""];
    let mut iter_over = input_over.into_iter();
    let res = parse_quoted(&mut iter_over, "test", 5);
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("length must be <="));
}

#[tokio::test]
async fn test_get_response_math_eval_complex() {
    let store = Arc::new(Mutex::new(vec![Resp {
        values: vec!["key".to_string()],
        probs: vec!["val".to_string()],
    }]));

    assert_eq!(
        get_response(&store, "10 * (2 + 3)").await,
        Some("50".to_string())
    );
    assert_eq!(
        get_response(&store, "100 / 4").await,
        Some("25".to_string())
    );
    assert_eq!(get_response(&store, "10 +").await, None);
}

#[tokio::test]
async fn test_get_response_punctuation_stripping() {
    let store = Arc::new(Mutex::new(vec![Resp {
        values: vec!["인사".to_string()],
        probs: vec!["답변".to_string()],
    }]));

    assert_eq!(
        get_response(&store, "인사!").await,
        Some("답변".to_string())
    );
    assert_eq!(
        get_response(&store, "인사?").await,
        Some("답변".to_string())
    );
    assert_eq!(
        get_response(&store, "인사...").await,
        Some("답변".to_string())
    );
    assert_eq!(
        get_response(&store, "인사-").await,
        Some("답변".to_string())
    );
}

#[test]
fn test_calculate_match_score_extremes() {
    let s_zeros = calculate_match_score(0, 0);
    assert!(s_zeros <= 100);

    let s_max = calculate_match_score(u64::MAX, u64::MAX);
    assert!(s_max <= 100);

    let s_zero_one = calculate_match_score(0, 1);
    assert!(s_zero_one <= 100);
}

#[test]
fn test_parse_quoted_single_quote_error() {
    let input = vec!["\""];
    let mut iter = input.into_iter();
    let res = parse_quoted(&mut iter, "test", 100);
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("missing closing quote"));
}

#[test]
fn test_parse_quoted_chained_tokens() {
    let input = vec!["\"hello", "world\""];
    let mut iter = input.into_iter();
    let first = iter.next().unwrap();
    let mut chained = std::iter::once(first).chain(&mut iter);
    let res = parse_quoted(&mut chained, "footer", 100);
    assert_eq!(res, Ok("hello world".to_string()));
}

#[test]
fn test_message_pair_target_for() {
    use crate::db::models::MessagePair;
    let pair = MessagePair {
        user: 100,
        bot: 200,
    };
    assert_eq!(pair.target_for(100), 200);
    assert_eq!(pair.target_for(200), 100);
    assert_eq!(pair.target_for(999), 100);
}
