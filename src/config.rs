use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::header::{
    HeaderMap, HeaderName, HeaderValue, ACCEPT, CONTENT_TYPE, ORIGIN, REFERER, USER_AGENT,
};
use std::collections::{HashMap, HashSet};

pub const DEFAULT_OUT_DIR: &str = "./generated_wiki_data";
pub const MAX_LIST_CONCUR: usize = 20;
pub const MAX_DETAIL_CONCUR: usize = 30;
pub const MAX_BULK_CONCUR: usize = 50;
pub const MAX_CALENDAR_CONCUR: usize = 5;

pub const HTTP_TIMEOUT_SECONDS: u64 = 35;
pub const HTTP_CONNECT_TIMEOUT: u64 = 20;
pub const MAX_RETRIES: u32 = 3;
pub const RETRY_DELAY_BASE_SECS: f32 = 1.5;

const BASE_API_URL: &str = "https://sg-wiki-api-static.hoyolab.com/hoyowiki/genshin/wapi";
pub const PAGE_SIZE: i64 = 50;
pub const BULK_BATCH_SIZE: usize = 50;

pub static API_ENDPOINTS: Lazy<HashMap<&'static str, String>> = Lazy::new(|| {
    HashMap::from([
        ("nav", format!("{}/home/navigation", BASE_API_URL)),
        ("list", format!("{}/get_entry_page_list", BASE_API_URL)),
        ("detail", format!("{}/entry_page", BASE_API_URL)),
        ("bulk", format!("{}/entry_pages", BASE_API_URL)),
        ("calendar", format!("{}/home/calendar", BASE_API_URL)),
    ])
});

pub static SUPPORTED_LANGS: Lazy<Vec<String>> = Lazy::new(|| {
    vec![
        "de-de", "en-us", "es-es", "fr-fr", "id-id", "it-it", "ja-jp", "ko-kr", "pt-pt", "ru-ru",
        "th-th", "tr-tr", "vi-vn", "zh-cn", "zh-tw",
    ]
    .into_iter()
    .map(String::from)
    .collect()
});

const ANDROID_VER: &str = "11";
const DEVICE_MODEL: &str = "Pixel 5";
const BUILD_ID: &str = "RQ3A.211001.001";
const CHROME_VER: &str = "107.0.0.0";
const WEBKIT_VER: &str = "537.36";
static USER_AGENT_VAL: Lazy<String> = Lazy::new(|| {
    format!("Mozilla/5.0 (Linux; Android {}; {} Build/{}; wv) AppleWebKit/{} (KHTML, like Gecko) Version/4.0 Chrome/{} Mobile Safari/{}", ANDROID_VER, DEVICE_MODEL, BUILD_ID, WEBKIT_VER, CHROME_VER, WEBKIT_VER)
});
const ORIGIN_VAL: &str = "https://www.hoyolab.com";
const REFERER_VAL: &str = "https://www.hoyolab.com/";

pub static BASE_UA_HEADERS: Lazy<HeaderMap> = Lazy::new(|| {
    let mut h = HeaderMap::new();
    h.insert(USER_AGENT, HeaderValue::from_static(&USER_AGENT_VAL));
    h.insert(ORIGIN, HeaderValue::from_static(ORIGIN_VAL));
    h.insert(REFERER, HeaderValue::from_static(REFERER_VAL));

    h.insert(
        HeaderName::from_static("x-rpc-device_name"),
        HeaderValue::from_static("Google%20Pixel%205"),
    );
    h.insert(
        HeaderName::from_static("x-rpc-device_model"),
        HeaderValue::from_static("Pixel 5"),
    );
    h.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    h.insert(
        ACCEPT,
        HeaderValue::from_static("application/json, text/plain, */*"),
    );
    h.insert(
        HeaderName::from_static("x-rpc-client_type"),
        HeaderValue::from_static("4"),
    );
    h.insert(
        HeaderName::from_static("x-rpc-app_version"),
        HeaderValue::from_static("1.5.0"),
    );
    h
});

pub const MAX_RECURSION_DEPTH: u32 = 15;

pub const KEY_CHAR_VISION: &str = "character_vision";
pub const KEY_CHAR_RARITY: &str = "character_rarity";
pub const KEY_WEAPON_RARITY: &str = "weapon_rarity";

pub const KEY_CHAR_WEAPON: &str = "character_weapon";
pub const KEY_CHAR_REGION: &str = "character_region";
pub const KEY_WEAPON_TYPE: &str = "weapon_type";

pub static LIST_FILTER_FIELDS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        KEY_CHAR_VISION,
        KEY_CHAR_REGION,
        KEY_CHAR_WEAPON,
        KEY_CHAR_RARITY,
        "character_property",
        KEY_WEAPON_TYPE,
        "weapon_property",
        KEY_WEAPON_RARITY,
        "reliquary_effect",
        "object_type",
        "card_character_camp",
        "card_character_obtaining_method",
        "card_character_charging_point",
        "card_character_weapon_type",
        "card_character_element",
        "card_character_arkhe",
    ]
    .iter()
    .cloned()
    .collect()
});

pub static MULTI_VALUE_FILTER_FIELDS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    ["card_character_camp", "object_type", "reliquary_effect"]
        .iter()
        .cloned()
        .collect()
});

pub const COMPONENT_BASE_INFO: &str = "baseInfo";
pub const COMPONENT_ASCENSION: &str = "ascension";
pub const COMPONENT_TALENT: &str = "talent";
pub const COMPONENT_SUMMARY_LIST: &str = "summaryList";
pub const COMPONENT_STORY: &str = "story";
pub const COMPONENT_VOICE: &str = "voice";
pub const COMPONENT_TEXTUAL_RESEARCH: &str = "textual_research";
pub const COMPONENT_GALLERY_CHARACTER: &str = "gallery_character";
pub const COMPONENT_ARTIFACT_LIST: &str = "artifact_list";
pub const COMPONENT_RELIQUARY_SET_EFFECT: &str = "reliquary_set_effect";
pub const COMPONENT_MAP: &str = "map";
pub const COMPONENT_CUSTOMIZE: &str = "customize";
pub const COMPONENT_BODY: &str = "body";
pub const COMPONENT_TIMELINE: &str = "timeline";
pub const COMPONENT_VIDEO_COLLECTION: &str = "video_collection";
pub const COMPONENT_TCG: &str = "tcg";
pub const COMPONENT_DROP_MATERIAL: &str = "drop_material";

pub static HEADING_TAGS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    ["h1", "h2", "h3", "h4", "h5", "h6"]
        .iter()
        .cloned()
        .collect()
});
pub static HTML_STRIP_TAGS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    ["style", "script", "meta", "link"]
        .iter()
        .cloned()
        .collect()
});
pub static HTML_BLOCK_TAGS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "p",
        "div",
        "ul",
        "ol",
        "blockquote",
        "figure",
        "body",
        "html",
        "header",
        "footer",
        "section",
        "article",
        "aside",
        "table",
        "tbody",
        "tr",
        "td",
        "th",
        "pre",
    ]
    .iter()
    .cloned()
    .collect()
});
pub static HTML_INLINE_TAGS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "span", "a", "b", "i", "u", "em", "strong", "font", "mark", "small", "sub", "sup", "code",
        "img",
    ]
    .iter()
    .cloned()
    .collect()
});
pub static TARGET_HTML_CUSTOM_TAGS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "custom-entry",
        "custom-image",
        "custom-ruby",
        "custom-post",
        "custom-video",
        "custom-map",
    ]
    .iter()
    .cloned()
    .collect()
});

pub static FORBIDDEN_CHARS_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"[<>:"/\\|?*\x00-\x1f\x7f\^!@#$%^&*()+={}\[\];,.'’]"#).unwrap());
pub static WHITESPACE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[\s_]+").unwrap());

pub static RE_ADJACENT_SAME_CLR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"</color=#[0-9A-Fa-f]{6,8}><color=#([0-9A-Fa-f]{6,8})>").unwrap());

pub static RE_ADJACENT_CLR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"</color><color=#([0-9A-Fa-f]{6,8})>").unwrap());
pub static RE_EMPTY_COLOR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"<color=#[0-9A-Fa-f]{6,8}>\s*</color>").unwrap());
