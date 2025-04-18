use crate::config;
use crate::model::common::{
    deserialize_flexible_i64, deserialize_optional_flexible_i64, deserialize_optional_string,
    deserialize_string_or_default, EntryId, MenuId,
};
use serde::de::{self, DeserializeOwned, Deserializer, MapAccess, Visitor};
use serde::Deserialize;
use serde_json::{from_str, from_value, Value};
use std::collections::HashMap;
use std::fmt;

// --- Struct definitions (ApiWrapper, ApiNavResponse, etc.) remain the same ---
#[derive(Deserialize, Debug, Clone)]
pub struct ApiWrapper<T> {
    #[serde(deserialize_with = "deserialize_flexible_i64")]
    pub retcode: i64,
    #[serde(default)]
    pub message: String,
    pub data: Option<T>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ApiNavResponse {
    #[serde(default)]
    pub nav: Vec<ApiNavEntry>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct ApiNavEntry {
    #[serde(default)]
    pub menu: Option<ApiNavMenu>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub icon_url: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct ApiNavMenu {
    #[serde(deserialize_with = "deserialize_flexible_i64")]
    pub menu_id: MenuId,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ApiListResponse {
    #[serde(default, deserialize_with = "deserialize_optional_flexible_i64")]
    pub total: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_value_to_vec_list_item")]
    pub list: Vec<ApiListItem>,
}

fn deserialize_value_to_vec_list_item<'de, D>(deserializer: D) -> Result<Vec<ApiListItem>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Array(arr) => from_value(Value::Array(arr)).map_err(de::Error::custom),
        _ => Ok(Vec::new()),
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct ApiListItem {
    #[serde(deserialize_with = "deserialize_flexible_i64")]
    pub entry_page_id: EntryId,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub name: String,
    #[serde(
        default,
        alias = "icon",
        deserialize_with = "deserialize_string_or_default"
    )]
    pub icon_url: String,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub desc: Option<String>,
    #[serde(default)]
    pub display_field: Option<Value>,
    #[serde(default, deserialize_with = "deserialize_value_to_value_object")]
    pub filter_values: Value,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ApiDetailResponse {
    #[serde(bound(deserialize = "'de: 'de"))]
    pub page: ApiDetailPage,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct ApiDetailPage {
    #[serde(default, deserialize_with = "deserialize_optional_flexible_i64")]
    pub id: Option<EntryId>,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub name: String,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub desc: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub icon_url: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub header_img_url: Option<String>,
    #[serde(default)]
    pub modules: Vec<ApiModule>,
    #[serde(default, deserialize_with = "deserialize_value_to_value_object")]
    pub filter_values: Value,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub menu_name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_flexible_i64")]
    pub menu_id: Option<MenuId>,
    #[serde(default, deserialize_with = "deserialize_optional_flexible_i64")]
    pub version: Option<i64>,
}

fn deserialize_value_to_value_object<'de, D>(deserializer: D) -> Result<Value, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    if value.is_object() {
        Ok(value)
    } else {
        Ok(Value::Object(serde_json::Map::new()))
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct ApiModule {
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub name: Option<String>,
    #[serde(default)]
    pub components: Vec<ApiComponent>,
    pub id: Option<String>,
    pub is_poped: Option<bool>,
    pub is_customize_name: Option<bool>,
    pub is_abstract: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_value_to_vec_module")]
    pub modules: Vec<ApiModule>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct ApiBaseInfoItem {
    #[serde(default)]
    pub key: String,
    #[serde(default, deserialize_with = "deserialize_string_or_value")]
    pub value: Value,
    #[serde(default, alias = "isMaterial")]
    pub is_material: Option<bool>,
    pub id: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct ApiAscensionItem {
    #[serde(default)]
    pub key: String,
    #[serde(
        alias = "combatList",
        default = "default_value_null",
        deserialize_with = "deserialize_null_or_value"
    )]
    pub combat_list: Value,
    #[serde(
        default = "default_value_null",
        deserialize_with = "deserialize_string_array_or_null"
    )]
    pub materials: Value,
    pub id: Option<String>,
}

fn deserialize_null_or_value<'de, D>(deserializer: D) -> Result<Value, D::Error>
where
    D: Deserializer<'de>,
{
    let v = Value::deserialize(deserializer)?;
    if v.is_null() {
        Ok(default_value_null())
    } else {
        Ok(v)
    }
}

fn default_value_null() -> Value {
    Value::Null
}

fn deserialize_string_array_or_null<'de, D>(deserializer: D) -> Result<Value, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringArrayOrNull {
        Strings(Vec<String>),
        Null(serde::de::IgnoredAny),
    }

    match StringArrayOrNull::deserialize(deserializer)? {
        StringArrayOrNull::Strings(s) => {
            Ok(Value::Array(s.into_iter().map(Value::String).collect()))
        }
        StringArrayOrNull::Null(_) => Ok(Value::Null),
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct ApiTalentItem {
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub key: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub title: Option<String>,
    #[serde(
        default,
        alias = "icon_url",
        deserialize_with = "deserialize_optional_string"
    )]
    pub icon_url: Option<String>,
    #[serde(default, deserialize_with = "deserialize_string_or_value")]
    pub desc: Value,
    #[serde(
        alias = "attributes",
        default = "default_value_null",
        deserialize_with = "deserialize_null_or_value"
    )]
    pub attributes: Value,
    #[serde(default, deserialize_with = "deserialize_string_or_value")]
    pub materials: Value,
    #[serde(
        default,
        alias = "talent_img",
        deserialize_with = "deserialize_optional_string"
    )]
    pub talent_img: Option<String>,
    pub id: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct ApiSummaryItem {
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub icon_url: String,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub name: String,
    #[serde(default, deserialize_with = "deserialize_string_or_value")]
    pub desc: Value,
    pub id: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct ApiStoryItem {
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub title: String,
    #[serde(default, deserialize_with = "deserialize_string_or_value")]
    pub desc: Value,
    pub id: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct ApiBodyItem {
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub title: String,
    #[serde(default, deserialize_with = "deserialize_string_or_value")]
    pub content: Value,
    pub id: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct ApiVoiceItem {
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub title: String,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub desc: String,
    #[serde(default)]
    pub audios: Option<Value>,
    pub id: Option<String>,
}

// This is the struct we expect to deserialize *from* the API data field
#[derive(Deserialize, Debug, Clone, Default)] // Added Default
pub struct GalleryCharacterWrapper {
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub pic: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_to_default_vec")]
    pub list: Vec<ApiGalleryCharacterItem>,
}

// This represents one item *within* the gallery list from the API
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct ApiGalleryCharacterItem {
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub key: String,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub img: String,
    #[serde(
        default,
        alias = "imgDesc",
        deserialize_with = "deserialize_string_or_value"
    )]
    pub img_desc: Value,
    pub id: Option<String>,
}

fn deserialize_null_to_default_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let v: Value = Deserialize::deserialize(deserializer)?;
    match v {
        Value::Null => Ok(Vec::new()),
        _ => from_value(v).map_err(de::Error::custom),
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct ApiArtifactListItem {
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub position: String,
    #[serde(default, deserialize_with = "deserialize_string_or_value")]
    pub desc: Value,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub icon_url: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct ApiReliquaryEffect {
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub two_set_effect: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub four_set_effect: Option<String>,
    pub id: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct ApiMapData {
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub url: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct ApiTextualResearchItem {
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub title: String,
    #[serde(default, deserialize_with = "deserialize_string_or_value")]
    pub desc: Value,
    #[serde(default)]
    pub audios: Option<Value>,
    pub id: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct ApiTimelineModuleContent {
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub desc: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct ApiTimelineEvent {
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub event_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub title: String,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub sub_title: String,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub desc: String,
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub icon_url: String,
    #[serde(default)]
    pub modules: Vec<ApiTimelineModuleContent>,
}

#[derive(Deserialize, Debug, Clone, Default)] // Added Default
#[serde(rename_all = "snake_case")]
pub struct ApiTimelineListData {
    #[serde(default)]
    pub list: Vec<ApiTimelineEvent>,
}

// Specific struct to parse the *inner* data of the video_collection component
#[derive(Deserialize, Debug, Clone)]
pub struct ApiVideoCollectionDataList {
    #[serde(default)]
    pub list: Vec<ApiVideoCategory>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ApiVideoCategory {
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub name: String,
    #[serde(default)]
    pub videos: Vec<ApiVideoCollectionItem>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ApiVideoCollectionItem {
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub name: String, // The name field within the video object itself (if any)
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub title: String,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub video_id: Option<String>, // Renamed from 'id' to avoid confusion
    #[serde(default, deserialize_with = "deserialize_string_or_default")]
    pub url: String,
    #[serde(
        default,
        alias = "img",
        deserialize_with = "deserialize_string_or_default"
    )]
    pub cover: String,
    #[serde(default, deserialize_with = "deserialize_flexible_i64")]
    pub duration: i64,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct ApiTcgHeaderImage {
    #[serde(default)]
    pub img_url: String,
    #[serde(default)]
    pub img_desc: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct ApiTcgData {
    #[serde(default)]
    pub cost_icon_type: String,
    #[serde(default)]
    pub cost_icon_type_any: String,
    #[serde(default)]
    pub header_imgs: Vec<ApiTcgHeaderImage>,
    #[serde(default)]
    pub hp: i64,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "snake_case")]
pub struct ApiDropMaterialData {
    #[serde(default, deserialize_with = "deserialize_string_vec_or_null_as_empty")]
    pub list: Vec<String>,
}

fn deserialize_string_vec_or_null_as_empty<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<Vec<String>>::deserialize(deserializer).map(|opt_vec| opt_vec.unwrap_or_default())
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum ApiComponentData {
    BaseInfoList(Vec<ApiBaseInfoItem>),
    AscensionList(Vec<ApiAscensionItem>),
    TalentList(Vec<ApiTalentItem>),
    SummaryList(Vec<ApiSummaryItem>),
    StoryList(Vec<ApiStoryItem>),
    BodyList(Vec<ApiBodyItem>),
    VoiceList(Vec<ApiVoiceItem>),
    GalleryCharacterList(Vec<ApiGalleryCharacterItem>),
    GalleryCharacterWrapperData(GalleryCharacterWrapper), // Parsed wrapper
    ArtifactMap(HashMap<String, ApiArtifactListItem>),
    ReliquarySetEffect(ApiReliquaryEffect),
    Map(ApiMapData),
    Customize(Value), // Stores the raw value (string or object)
    TextualResearchList(Vec<ApiTextualResearchItem>),
    Timeline(ApiTimelineListData),
    VideoCollection(Value), // Store raw Value for video collection initially
    Tcg(ApiTcgData),
    DropMaterial(ApiDropMaterialData),
    Unknown(Value),
}

#[derive(Debug, Clone)]
pub struct ApiComponent {
    pub component_id: String,
    pub typed_data: ApiComponentData,
}

impl<'de> Deserialize<'de> for ApiComponent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            component_id: String,
            #[serde(default, deserialize_with = "deserialize_string_or_value")]
            data: Value,
        }

        let helper = Helper::deserialize(deserializer)?;
        let data_val = helper.data;
        let component_id = helper.component_id;

        #[derive(Deserialize)]
        struct ListWrapper<T> {
            list: Vec<T>,
        }
        #[derive(Deserialize)]
        struct ContentWrapper<T> {
            content: Vec<T>,
        }

        fn parse_direct_component<'a, T>(data_val: &'a Value) -> Result<T, serde_json::Error>
        where
            T: DeserializeOwned,
        {
            match data_val {
                Value::String(s) => from_str::<T>(s.trim()), // Trim string before parsing
                _ => from_value::<T>(data_val.clone()),
            }
        }

        // Function to attempt parsing Vec<T> from string or value, checking for "list"/"content" keys
        fn parse_list_component<'a, T>(data_val: &'a Value) -> Result<Vec<T>, serde_json::Error>
        where
            T: DeserializeOwned,
        {
            match data_val {
                Value::String(s) => {
                    let trimmed = s.trim();
                    from_str::<ListWrapper<T>>(trimmed)
                        .map(|w| w.list)
                        .or_else(|_| from_str::<ContentWrapper<T>>(trimmed).map(|w| w.content))
                        .or_else(|_| from_str::<Vec<T>>(trimmed))
                        .or_else(|_e| Ok(Vec::new())) // Return empty on error
                }
                Value::Object(map) => {
                    if let Some(list_val) = map.get("list") {
                        from_value::<Vec<T>>(list_val.clone())
                    } else if let Some(content_val) = map.get("content") {
                        from_value::<Vec<T>>(content_val.clone())
                    } else {
                        Ok(Vec::new())
                    }
                }
                Value::Array(_) => from_value::<Vec<T>>(data_val.clone()),
                _ => Ok(Vec::new()),
            }
        }

        let typed_data_result =
            match component_id.as_str() {
                config::COMPONENT_BASE_INFO => parse_list_component::<ApiBaseInfoItem>(&data_val)
                    .map(ApiComponentData::BaseInfoList),
                config::COMPONENT_ASCENSION => parse_list_component::<ApiAscensionItem>(&data_val)
                    .map(ApiComponentData::AscensionList),
                config::COMPONENT_TALENT => parse_list_component::<ApiTalentItem>(&data_val)
                    .map(ApiComponentData::TalentList),
                config::COMPONENT_SUMMARY_LIST => parse_list_component::<ApiSummaryItem>(&data_val)
                    .map(ApiComponentData::SummaryList),
                config::COMPONENT_STORY => {
                    parse_list_component::<ApiStoryItem>(&data_val).map(ApiComponentData::StoryList)
                }
                config::COMPONENT_BODY => {
                    parse_list_component::<ApiBodyItem>(&data_val).map(ApiComponentData::BodyList)
                }
                config::COMPONENT_VOICE => {
                    parse_list_component::<ApiVoiceItem>(&data_val).map(ApiComponentData::VoiceList)
                }
                config::COMPONENT_GALLERY_CHARACTER => {
                    match parse_direct_component::<GalleryCharacterWrapper>(&data_val) {
                        Ok(wrapper) => Ok(ApiComponentData::GalleryCharacterWrapperData(wrapper)),
                        Err(_) => {
                            parse_list_component::<ApiGalleryCharacterItem>(&data_val)
                                .map(ApiComponentData::GalleryCharacterList)
                                .or_else(|_| Ok(ApiComponentData::GalleryCharacterList(vec![])))
                        }
                    }
                }
                config::COMPONENT_ARTIFACT_LIST => {
                    parse_direct_component::<HashMap<String, ApiArtifactListItem>>(&data_val)
                        .map(ApiComponentData::ArtifactMap)
                }
                config::COMPONENT_RELIQUARY_SET_EFFECT => {
                    parse_direct_component::<ApiReliquaryEffect>(&data_val)
                        .map(ApiComponentData::ReliquarySetEffect)
                }
                config::COMPONENT_MAP => {
                    parse_direct_component::<ApiMapData>(&data_val).map(ApiComponentData::Map)
                }
                config::COMPONENT_CUSTOMIZE => Ok(ApiComponentData::Customize(data_val.clone())),
                config::COMPONENT_TEXTUAL_RESEARCH => {
                    parse_list_component::<ApiTextualResearchItem>(&data_val)
                        .map(ApiComponentData::TextualResearchList)
                }
                config::COMPONENT_TIMELINE => {
                    if data_val.is_null() {
                        Ok(ApiComponentData::Timeline(ApiTimelineListData::default()))
                    } else {
                        parse_direct_component::<ApiTimelineListData>(&data_val)
                            .map(ApiComponentData::Timeline)
                    }
                }
                config::COMPONENT_VIDEO_COLLECTION => {
                    Ok(ApiComponentData::VideoCollection(data_val.clone()))
                }
                config::COMPONENT_TCG => {
                    parse_direct_component::<ApiTcgData>(&data_val).map(ApiComponentData::Tcg)
                }
                config::COMPONENT_DROP_MATERIAL => {
                    if data_val.is_null() {
                        Ok(ApiComponentData::DropMaterial(
                            ApiDropMaterialData::default(),
                        ))
                    } else {
                        parse_direct_component::<ApiDropMaterialData>(&data_val)
                            .map(ApiComponentData::DropMaterial)
                    }
                }
                _ => Ok(ApiComponentData::Unknown(data_val.clone())),
            };

        let typed_data = typed_data_result.map_err(|e| {
            let data_preview = data_val.to_string().chars().take(100).collect::<String>();
            let err_msg = format!(
                "Failed to parse data for component_id '{}': {}. Raw data preview: {}",
                component_id, e, data_preview
            );
            de::Error::custom(err_msg)
        })?;

        Ok(ApiComponent {
            component_id,
            typed_data,
        })
    }
}

fn deserialize_value_to_vec_module<'de, D>(deserializer: D) -> Result<Vec<ApiModule>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Array(arr) => from_value(Value::Array(arr)).map_err(de::Error::custom),
        Value::Object(map) => {
            if let Some(modules_val) = map.get("modules").or_else(|| map.get("module")) {
                if modules_val.is_array() {
                    from_value(modules_val.clone()).map_err(de::Error::custom)
                } else {
                    Ok(Vec::new())
                }
            } else if map.contains_key("components") {
                from_value::<ApiModule>(Value::Object(map))
                    .map(|m| vec![m])
                    .map_err(de::Error::custom)
            } else {
                Ok(Vec::new())
            }
        }
        _ => Ok(Vec::new()),
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct ApiBulkResponse {
    #[serde(default)]
    pub entry_pages: Vec<ApiBulkPage>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ApiBulkPage {
    #[serde(deserialize_with = "deserialize_flexible_i64")]
    pub id: EntryId,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub desc: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub icon_url: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ApiCalendarResponse {
    #[serde(default)]
    pub calendar: Vec<Value>,
    #[serde(default)]
    pub op: Vec<Value>,
}

// Keep this function as is: it should NOT try to parse JSON in strings itself.
pub fn deserialize_string_or_value<'de, D>(deserializer: D) -> Result<Value, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringOrValueVisitor;

    impl<'de> Visitor<'de> for StringOrValueVisitor {
        type Value = Value;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("string, object, array, or null")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if value.is_empty() {
                return Ok(Value::Null);
            }
            Ok(Value::String(value.to_string()))
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if value.is_empty() {
                return Ok(Value::Null);
            }
            Ok(Value::String(value))
        }

        fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            let deserialized_map =
                serde::Deserialize::deserialize(de::value::MapAccessDeserializer::new(map))?;
            Ok(Value::Object(deserialized_map))
        }

        fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            let deserialized_seq =
                serde::Deserialize::deserialize(de::value::SeqAccessDeserializer::new(seq))?;
            Ok(Value::Array(deserialized_seq))
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Value::Null)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Value::Null)
        }

        fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Value::Bool(v))
        }
        fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Value::from(v))
        }
        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Value::from(v))
        }
        fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Value::from(v))
        }
    }
    deserializer.deserialize_any(StringOrValueVisitor)
}
