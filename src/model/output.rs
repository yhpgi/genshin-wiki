use super::common::{EntryId, MenuId};
use super::html::HtmlNode;
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(untagged, rename_all = "camelCase")]
pub enum FilterValue {
    Single(String),
    Multiple(Vec<String>),
    Integer(i64),
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OutputListItem {
    #[serde(rename = "epId")]
    pub id: EntryId,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub icon_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub filter_values: HashMap<String, FilterValue>,
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OutputListFile {
    #[serde(
        serialize_with = "chrono::serde::ts_seconds::serialize",
        deserialize_with = "chrono::serde::ts_seconds::deserialize"
    )]
    #[schemars(with = "i64")]
    pub version: DateTime<Utc>,
    pub language: String,
    pub menu_id: MenuId,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub menu_name: String,
    pub total_items: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub list: Vec<OutputListItem>,
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OutputNavMenuItem {
    pub menu_id: MenuId,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub icon_url: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct OutputBaseInfoItem {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Vec<HtmlNode>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_material: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct OutputAscensionItem {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub key: String,

    #[serde(default = "default_value_null")]
    pub combat_stats: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materials: Option<Vec<HtmlNode>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct OutputTalentItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub icon_url: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub desc: Vec<HtmlNode>,
    #[serde(default = "default_value_null")]
    pub attributes: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materials: Option<Vec<Option<Vec<HtmlNode>>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub talent_img: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct OutputSummaryItem {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub icon_url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub desc: Vec<HtmlNode>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct OutputStoryItem {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub desc: Vec<HtmlNode>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct AudioInfo {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct OutputVoiceItem {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub desc: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audios: Vec<AudioInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct OutputGalleryCharacterItem {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub key: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub img: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub img_desc: Vec<HtmlNode>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct OutputArtifactListItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub position: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub desc: Vec<HtmlNode>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub icon_url: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct OutputReliquaryEffect {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub two_set_effect: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub four_set_effect: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct OutputTextualResearchItem {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub desc: Vec<HtmlNode>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct OutputTimelineEvent {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sub_title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub bg_url: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contents: Vec<HtmlNode>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct OutputVideoCollectionItem {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub video_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cover_url: String,
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub duration: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct OutputTcgHeaderImage {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub img_url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub img_desc: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct OutputTcgData {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cost_icon_type: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cost_icon_type_any: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub header_imgs: Vec<OutputTcgHeaderImage>,
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub hp: i64,
}

fn is_zero_i64(num: &i64) -> bool {
    *num == 0
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(untagged)]
pub enum ComponentData {
    BaseInfo(Vec<OutputBaseInfoItem>),
    Ascension(Vec<OutputAscensionItem>),
    Talent(Vec<OutputTalentItem>),
    SummaryList(Vec<OutputSummaryItem>),
    Story(Vec<OutputStoryItem>),
    Voice(Vec<OutputVoiceItem>),
    GalleryCharacter(Vec<OutputGalleryCharacterItem>),
    ArtifactList(HashMap<String, OutputArtifactListItem>),
    ReliquarySetEffect(OutputReliquaryEffect),
    MapUrl(String),
    TextualResearch(Vec<OutputTextualResearchItem>),
    Timeline(Vec<OutputTimelineEvent>),
    VideoCollection(Vec<OutputVideoCollectionItem>),
    Customize(Vec<HtmlNode>),
    Tcg(OutputTcgData),
    DropMaterial(Vec<HtmlNode>),
    Unknown(Value),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct OutputDetailPage {
    #[serde(rename = "epId")]
    pub id: EntryId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header_img_url: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub filter_values: HashMap<String, FilterValue>,
    pub menu_id: MenuId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub menu_name: Option<String>,
    pub version: i64,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub components: HashMap<String, ComponentData>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OutputCalendarAbstract {
    #[serde(rename = "epId")]
    pub id: EntryId,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub icon_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character_vision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character_rarity: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weapon_rarity: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OutputCalendarItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drop_day: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub break_type: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obtain_method: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub character_abstracts: Vec<OutputCalendarAbstract>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub material_abstracts: Vec<OutputCalendarAbstract>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ep_abstracts: Vec<OutputCalendarAbstract>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OutputCalendarOpItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_birth: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ep_abstracts: Vec<OutputCalendarAbstract>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OutputCalendarFile {
    #[serde(
        serialize_with = "chrono::serde::ts_seconds::serialize",
        deserialize_with = "chrono::serde::ts_seconds::deserialize"
    )]
    #[schemars(with = "i64")]
    pub version: DateTime<Utc>,
    pub language: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub calendar: Vec<OutputCalendarItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub op: Vec<OutputCalendarOpItem>,
}

fn default_value_null() -> Value {
    Value::Null
}
