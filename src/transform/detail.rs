use crate::api::model;
use crate::api::model::{
    ApiComponentData, ApiDropMaterialData, ApiTcgData, ApiTimelineListData,
    ApiVideoCollectionDataList,
};
use crate::error::AppResult;
use crate::logging::{log, LogLevel};
use crate::model::common::EntryId;
use crate::model::html;
use crate::model::html::HtmlNode;
use crate::model::output::{
    self, AudioInfo, ComponentData, OutputArtifactListItem, OutputAscensionItem,
    OutputBaseInfoItem, OutputDetailPage, OutputGalleryCharacterItem, OutputReliquaryEffect,
    OutputStoryItem, OutputSummaryItem, OutputTalentItem, OutputTcgData, OutputTcgHeaderImage,
    OutputTextualResearchItem, OutputTimelineEvent, OutputVideoCollectionItem, OutputVoiceItem,
};
use crate::transform::bulk::BulkStore;
use crate::transform::{common, html_parser};
use crate::utils;
use async_recursion::async_recursion;
use chrono::Utc;
use serde_json::Value;
use serde_json::{from_str, from_value};
use std::collections::{hash_map::Entry, HashMap, HashSet};
use std::ops::Not;
use std::sync::Arc;
use tokio::task::JoinSet;

#[async_recursion]
pub async fn transform_detail_page(
    raw_page: model::ApiDetailPage,
    bulk_store: Arc<BulkStore>,
    lang: &str,
) -> AppResult<Option<OutputDetailPage>> {
    let page_id = match raw_page.id {
        Some(id) if id > 0 => id,
        _ => return Ok(None),
    };
    let version = raw_page.version.unwrap_or_else(|| Utc::now().timestamp());
    let menu_id = raw_page.menu_id.unwrap_or(0);

    let final_name = bulk_store
        .get_name(page_id)
        .map(String::from)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            raw_page
                .name
                .is_empty()
                .not()
                .then_some(raw_page.name.clone())
        });

    let final_icon_url = bulk_store
        .get_icon(page_id)
        .map(String::from)
        .filter(|s| !s.is_empty())
        .or_else(|| raw_page.icon_url.clone());

    let final_desc = bulk_store
        .get_desc(page_id)
        .map(String::from)
        .or_else(|| raw_page.desc.clone());

    let filter_values = common::process_filters_value(&raw_page.filter_values);

    let mut component_tasks = JoinSet::new();
    let all_components = flatten_modules_components(raw_page.modules);

    for component in all_components {
        let bulk_store_clone = bulk_store.clone();
        let lang_clone = lang.to_string();
        component_tasks.spawn(async move {
            transform_component_content(component, page_id, bulk_store_clone, &lang_clone).await
        });
    }

    let mut final_components: HashMap<String, ComponentData> = HashMap::new();
    while let Some(result) = component_tasks.join_next().await {
        match result {
            Ok(Ok(Some((id, new_content)))) => {
                let camel_case_id = common::to_camel_case(&id);
                match final_components.entry(camel_case_id.clone()) {
                    Entry::Occupied(mut entry) => {
                        let existing_content_mut = entry.get_mut();

                        match (existing_content_mut, new_content) {
                            (ComponentData::BaseInfo(existing), ComponentData::BaseInfo(new)) => {
                                existing.extend(new)
                            }
                            (ComponentData::Ascension(existing), ComponentData::Ascension(new)) => {
                                existing.extend(new)
                            }
                            (ComponentData::Talent(existing), ComponentData::Talent(new)) => {
                                existing.extend(new)
                            }
                            (
                                ComponentData::SummaryList(existing),
                                ComponentData::SummaryList(new),
                            ) => existing.extend(new),
                            (ComponentData::Story(existing), ComponentData::Story(new)) => {
                                existing.extend(new)
                            }
                            (ComponentData::Voice(existing), ComponentData::Voice(new)) => {
                                existing.extend(new)
                            }
                            (
                                ComponentData::TextualResearch(existing),
                                ComponentData::TextualResearch(new),
                            ) => existing.extend(new),
                            (ComponentData::Timeline(existing), ComponentData::Timeline(new)) => {
                                existing.extend(new)
                            }
                            (ComponentData::Customize(existing), ComponentData::Customize(new)) => {
                                existing.extend(new)
                            }
                            (
                                ComponentData::DropMaterial(existing),
                                ComponentData::DropMaterial(new),
                            ) => existing.extend(new),

                            (
                                ComponentData::GalleryCharacter(existing_items),
                                ComponentData::GalleryCharacter(new_items),
                            ) => {
                                let existing_keys: HashSet<String> =
                                    existing_items.iter().map(|item| item.key.clone()).collect();
                                let existing_img: HashSet<String> =
                                    existing_items.iter().map(|item| item.img.clone()).collect();
                                for new_item in new_items {
                                    if !existing_img.contains(&new_item.img) {
                                        if !existing_keys.contains(&new_item.key) {
                                            existing_items.push(new_item.clone());
                                        }
                                    }
                                }
                            }

                            (
                                ComponentData::VideoCollection(existing_videos),
                                ComponentData::VideoCollection(new_videos),
                            ) => {
                                existing_videos.extend(new_videos);
                                existing_videos.sort_by(|a, b| a.video_id.cmp(&b.video_id));
                                existing_videos.dedup_by(|a, b| a.video_id == b.video_id);
                            }

                            (
                                ComponentData::ArtifactList(existing),
                                ComponentData::ArtifactList(new),
                            ) => existing.extend(new),

                            (existing, new) => {
                                if std::mem::discriminant(existing) != std::mem::discriminant(&new)
                                    && !matches!(existing, ComponentData::Unknown(_))
                                {
                                    log(LogLevel::Warning, &format!("Merging different component types for ID [{} / {}]: {}. Existing: {:?}, New: {:?}. Overwriting.", lang, page_id, camel_case_id, existing.discriminant_str(), new.discriminant_str()));
                                } else if !matches!(
                                    new,
                                    ComponentData::ReliquarySetEffect(_)
                                        | ComponentData::MapUrl(_)
                                        | ComponentData::Tcg(_)
                                        | ComponentData::Unknown(_)
                                ) {
                                    log(LogLevel::Warning, &format!("Overwriting component data for ID [{} / {}]: {}. Type: {:?}.", lang, page_id, camel_case_id, new.discriminant_str()));
                                }
                                *existing = new;
                            }
                        }
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(new_content);
                    }
                }
            }
            Ok(Ok(None)) => {}
            Ok(Err(e)) => log(
                LogLevel::Warning,
                &format!("Comp transform error [{} / {}]: {:?}", lang, page_id, e),
            ),
            Err(e) => log(
                LogLevel::Error,
                &format!(
                    "Comp transform task panicked [{} / {}]: {}",
                    lang, page_id, e
                ),
            ),
        }
    }

    final_components.retain(
        |_key, value| !matches!(value, ComponentData::Customize(nodes) if nodes.is_empty()),
    );

    if final_name.is_none() && final_components.is_empty() {
        Ok(None)
    } else {
        Ok(Some(OutputDetailPage {
            id: page_id,
            name: final_name,
            desc: final_desc,
            icon_url: final_icon_url,
            header_img_url: raw_page.header_img_url,
            components: final_components,
            filter_values,
            menu_id,
            menu_name: raw_page.menu_name,
            version,
        }))
    }
}

impl ComponentData {
    fn discriminant_str(&self) -> &'static str {
        match self {
            ComponentData::BaseInfo(_) => "BaseInfo",
            ComponentData::Ascension(_) => "Ascension",
            ComponentData::Talent(_) => "Talent",
            ComponentData::SummaryList(_) => "SummaryList",
            ComponentData::Story(_) => "Story",
            ComponentData::Voice(_) => "Voice",
            ComponentData::GalleryCharacter(_) => "GalleryCharacter",
            ComponentData::ArtifactList(_) => "ArtifactList",
            ComponentData::ReliquarySetEffect(_) => "ReliquarySetEffect",
            ComponentData::MapUrl(_) => "MapUrl",
            ComponentData::TextualResearch(_) => "TextualResearch",
            ComponentData::Timeline(_) => "Timeline",
            ComponentData::VideoCollection(_) => "VideoCollection",
            ComponentData::Customize(_) => "Customize",
            ComponentData::Tcg(_) => "Tcg",
            ComponentData::DropMaterial(_) => "DropMaterial",
            ComponentData::Unknown(_) => "Unknown",
        }
    }
}

fn flatten_modules_components(modules: Vec<model::ApiModule>) -> Vec<model::ApiComponent> {
    let mut components = Vec::new();
    for module in modules {
        components.extend(module.components);
        if !module.modules.is_empty() {
            components.extend(flatten_modules_components(module.modules));
        }
    }
    components
}

#[async_recursion]
async fn transform_component_content(
    api_comp: model::ApiComponent,
    page_id: EntryId,
    bulk_store: Arc<BulkStore>,
    lang: &str,
) -> AppResult<Option<(String, ComponentData)>> {
    let component_id = api_comp.component_id.clone();
    let typed_data = api_comp.typed_data;

    let output_component_data_result: AppResult<Option<ComponentData>> = match typed_data {
        ApiComponentData::BaseInfoList(items) => {
            transform_base_info_list(items, page_id, lang, &bulk_store)
                .await
                .map(|res| (!res.is_empty()).then_some(ComponentData::BaseInfo(res)))
        }
        ApiComponentData::AscensionList(items) => {
            transform_ascension_list(items, page_id, lang, &bulk_store)
                .await
                .map(|res| (!res.is_empty()).then_some(ComponentData::Ascension(res)))
        }
        ApiComponentData::TalentList(items) => {
            transform_talent_list(items, page_id, lang, &bulk_store)
                .await
                .map(|res| (!res.is_empty()).then_some(ComponentData::Talent(res)))
        }
        ApiComponentData::SummaryList(items) => {
            transform_summary_list(items, page_id, lang, &bulk_store)
                .await
                .map(|res| (!res.is_empty()).then_some(ComponentData::SummaryList(res)))
        }
        ApiComponentData::StoryList(items) => {
            transform_story_list(items, page_id, lang, &bulk_store)
                .await
                .map(|res| (!res.is_empty()).then_some(ComponentData::Story(res)))
        }
        ApiComponentData::BodyList(items) => {
            let mapped_items = items
                .into_iter()
                .map(|b| model::ApiStoryItem {
                    title: b.title,
                    desc: b.content,
                    id: b.id,
                })
                .collect();
            transform_story_list(mapped_items, page_id, lang, &bulk_store)
                .await
                .map(|res| (!res.is_empty()).then_some(ComponentData::Story(res)))
        }
        ApiComponentData::VoiceList(items) => transform_voice_list(items)
            .map(|res| (!res.is_empty()).then_some(ComponentData::Voice(res))),
        ApiComponentData::GalleryCharacterWrapperData(wrapper) => {
            transform_gallery_character_wrapper(wrapper, page_id, lang, &bulk_store)
                .await
                .map(|res| (!res.is_empty()).then_some(ComponentData::GalleryCharacter(res)))
        }
        ApiComponentData::GalleryCharacterList(items) => {
            transform_gallery_list(items, page_id, lang, &bulk_store)
                .await
                .map(|res| (!res.is_empty()).then_some(ComponentData::GalleryCharacter(res)))
        }
        ApiComponentData::ArtifactMap(item_map) => {
            transform_artifact_map(item_map, page_id, lang, &bulk_store)
                .await
                .map(|res| (!res.is_empty()).then_some(ComponentData::ArtifactList(res)))
        }
        ApiComponentData::ReliquarySetEffect(effect) => {
            Ok(transform_reliquary_effect(effect).map(ComponentData::ReliquarySetEffect))
        }
        ApiComponentData::Map(map_data) => {
            Ok((!map_data.url.is_empty()).then_some(ComponentData::MapUrl(map_data.url)))
        }
        ApiComponentData::Customize(cust_data_val) => {
            let mut html_string_to_parse: Option<String> = None;

            if let Value::Object(map) = &cust_data_val {
                html_string_to_parse = map.get("data").and_then(Value::as_str).map(String::from);
                if html_string_to_parse.is_none() {
                    log(LogLevel::Warning, &format!("Customize component [{} / {}] is object but missing 'data' key or data is not string: {:?}", lang, page_id, cust_data_val));
                }
            } else if let Value::String(outer_str) = &cust_data_val {
                let trimmed_outer_str = outer_str.trim();
                if !trimmed_outer_str.is_empty() {
                    match from_str::<HashMap<String, String>>(trimmed_outer_str) {
                        Ok(inner_map) => {
                            html_string_to_parse = inner_map.get("data").cloned();
                            if html_string_to_parse.is_none() {
                                log(LogLevel::Warning, &format!("Customize component [{} / {}] outer JSON string missing inner 'data' key. Outer string: '{}'", lang, page_id, trimmed_outer_str));
                            }
                        }
                        Err(_) => {
                            html_string_to_parse = Some(trimmed_outer_str.to_string());
                        }
                    }
                }
            } else if !cust_data_val.is_null() && cust_data_val != Value::String("".to_string()) {
                log(LogLevel::Warning, &format!("Unexpected structure/type for Customize component data [{} / {}]: {:?}. Skipping.", lang, page_id, cust_data_val));
            }

            if let Some(html_content) = html_string_to_parse {
                if html_content.trim().is_empty() {
                    Ok(None)
                } else {
                    parse_value_to_html_nodes(
                        &Value::String(html_content),
                        page_id,
                        lang,
                        &bulk_store,
                    )
                    .await
                    .map(|nodes| {
                        (!nodes.is_empty()).then_some(output::ComponentData::Customize(nodes))
                    })
                }
            } else {
                Ok(None)
            }
        }
        ApiComponentData::TextualResearchList(items) => {
            transform_textual_research(items, page_id, lang, &bulk_store)
                .await
                .map(|res| (!res.is_empty()).then_some(ComponentData::TextualResearch(res)))
        }
        ApiComponentData::Timeline(timeline_list_data) => {
            transform_timeline_component(timeline_list_data, page_id, lang, &bulk_store)
                .await
                .map(|res| (!res.is_empty()).then_some(ComponentData::Timeline(res)))
        }
        ApiComponentData::VideoCollection(data_val) => {
            transform_video_collection(data_val, page_id, lang)
                .await
                .map(|res| (!res.is_empty()).then_some(ComponentData::VideoCollection(res)))
        }
        ApiComponentData::Tcg(tcg_data) => {
            Ok(Some(ComponentData::Tcg(transform_tcg_data(tcg_data))))
        }
        ApiComponentData::DropMaterial(drop_data) => {
            transform_drop_material(drop_data, page_id, lang, &bulk_store)
                .await
                .map(|nodes| (!nodes.is_empty()).then_some(ComponentData::DropMaterial(nodes)))
        }
        ApiComponentData::Unknown(val) => {
            if val.is_null() {
                Ok(None)
            } else {
                log(
                    LogLevel::Warning,
                    &format!(
                        "Component '{}' [{} / {}] has Unknown data type. Storing raw.",
                        component_id, lang, page_id
                    ),
                );
                Ok(Some(ComponentData::Unknown(val)))
            }
        }
    };

    match output_component_data_result {
        Ok(Some(data)) => Ok(Some((component_id, data))),
        Ok(None) => Ok(None),
        Err(e) => {
            log(
                LogLevel::Warning,
                &format!(
                    "Failed to transform component '{}' [{} / {}]: {}",
                    component_id, lang, page_id, e
                ),
            );
            Err(e)
        }
    }
}

async fn transform_timeline_component(
    list_data: ApiTimelineListData,
    page_id: EntryId,
    lang: &str,
    bulk_store: &Arc<BulkStore>,
) -> AppResult<Vec<OutputTimelineEvent>> {
    let mut output_events = Vec::new();

    for event in list_data.list {
        let mut event_contents = Vec::new();
        for module_content in event.modules {
            let content_nodes = parse_value_to_html_nodes(
                &Value::String(module_content.desc),
                page_id,
                lang,
                bulk_store,
            )
            .await?;
            event_contents.extend(content_nodes);
        }

        if !event_contents.is_empty() && !event.title.is_empty() {
            let mut final_contents = Vec::with_capacity(event_contents.len() + 1);
            final_contents.push(HtmlNode::Heading {
                level: 3,
                text: event.title.clone(),
                alignment: None,
            });
            final_contents.extend(event_contents);
            event_contents = final_contents;
        }

        if !event_contents.is_empty() || !event.title.is_empty() {
            output_events.push(OutputTimelineEvent {
                title: event.title,
                sub_title: event.sub_title,
                bg_url: event.icon_url,
                contents: event_contents,
            });
        }
    }

    Ok(output_events)
}

async fn transform_video_collection(
    data_val: Value,
    page_id: EntryId,
    lang: &str,
) -> AppResult<Vec<OutputVideoCollectionItem>> {
    let mut output_videos: Vec<OutputVideoCollectionItem> = Vec::new();

    match data_val {
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() || trimmed == "null" {
                return Ok(vec![]);
            }
            match from_str::<ApiVideoCollectionDataList>(trimmed) {
                Ok(parsed_data) => {
                    for category in parsed_data.list {
                        for video_item in category.videos {
                            output_videos.push(OutputVideoCollectionItem {
                                video_id: video_item.video_id.unwrap_or_default(),
                                title: video_item.title,
                                url: video_item.url,
                                cover_url: video_item.cover,
                                duration: video_item.duration,
                            });
                        }
                    }
                }
                Err(e) => {
                    log(LogLevel::Warning, &format!("Failed to parse JSON string inside video_collection data for [{} / {}]: {}. String: '{}'", lang, page_id, e, trimmed));
                    return Ok(vec![]);
                }
            }
        }
        Value::Object(map) => {
            match from_value::<ApiVideoCollectionDataList>(Value::Object(map.clone())) {
                Ok(parsed_data) => {
                    for category in parsed_data.list {
                        for video_item in category.videos {
                            output_videos.push(OutputVideoCollectionItem {
                                video_id: video_item.video_id.unwrap_or_default(),
                                title: video_item.title,
                                url: video_item.url,
                                cover_url: video_item.cover,
                                duration: video_item.duration,
                            });
                        }
                    }
                }
                Err(e) => {
                    log(LogLevel::Warning, &format!("Failed to parse object video_collection data for [{} / {}]: {}. Object: {:?}", lang, page_id, e, map));
                    return Ok(vec![]);
                }
            }
        }
        Value::Null => {
            return Ok(vec![]);
        }
        _ => {
            log(
                LogLevel::Warning,
                &format!(
                    "Unexpected data type for video_collection for [{} / {}]: {:?}",
                    lang, page_id, data_val
                ),
            );
            return Ok(vec![]);
        }
    }

    output_videos.sort_by(|a, b| a.title.cmp(&b.title));
    output_videos.dedup_by(|a, b| a.url == b.url);

    Ok(output_videos)
}

fn transform_reliquary_effect(effect: model::ApiReliquaryEffect) -> Option<OutputReliquaryEffect> {
    if effect.two_set_effect.is_some() || effect.four_set_effect.is_some() {
        Some(OutputReliquaryEffect {
            two_set_effect: effect.two_set_effect,
            four_set_effect: effect.four_set_effect,
        })
    } else {
        None
    }
}

#[async_recursion]
async fn transform_textual_research(
    items: Vec<model::ApiTextualResearchItem>,
    page_id: EntryId,
    lang: &str,
    bulk_store: &Arc<BulkStore>,
) -> AppResult<Vec<OutputTextualResearchItem>> {
    let mut results = Vec::with_capacity(items.len());
    for item in items {
        let desc_nodes = parse_value_to_html_nodes(&item.desc, page_id, lang, bulk_store).await?;
        if !desc_nodes.is_empty() || !item.title.is_empty() {
            results.push(OutputTextualResearchItem {
                title: item.title,
                desc: desc_nodes,
            });
        }
    }
    Ok(results)
}

fn transform_tcg_data(api_data: ApiTcgData) -> OutputTcgData {
    OutputTcgData {
        cost_icon_type: api_data.cost_icon_type,
        cost_icon_type_any: api_data.cost_icon_type_any,
        header_imgs: api_data
            .header_imgs
            .into_iter()
            .map(|h| OutputTcgHeaderImage {
                img_url: h.img_url,
                img_desc: h.img_desc,
            })
            .collect(),
        hp: api_data.hp,
    }
}

async fn transform_drop_material(
    api_data: ApiDropMaterialData,
    page_id: EntryId,
    lang: &str,
    bulk_store: &Arc<BulkStore>,
) -> AppResult<Vec<HtmlNode>> {
    let mut results = Vec::new();

    for material_string in api_data.list {
        let value_node = Value::String(material_string);
        results.extend(parse_materials_value(&value_node, page_id, lang, bulk_store).await?);
    }
    Ok(results)
}

#[async_recursion]
async fn parse_value_to_html_nodes(
    value: &Value,
    page_id: EntryId,
    lang: &str,
    bulk_store: &Arc<BulkStore>,
) -> AppResult<Vec<HtmlNode>> {
    match value {
        Value::String(s) if s.trim().starts_with("$[") && s.trim().ends_with("]$") => {
            parse_materials_value(value, page_id, lang, bulk_store).await
        }
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                return Ok(vec![]);
            }

            if trimmed.contains('<') && trimmed.contains('>') {
                let html_owned = trimmed.to_string();
                let lang_owned = lang.to_string();
                let bulk_store_clone = bulk_store.clone();
                let parse_result = utils::run_blocking(move || {
                    html_parser::parse_html_content(&html_owned, page_id, &lang_owned)
                })
                .await;

                match parse_result {
                    Ok(nodes) => {
                        html_parser::post_process_html_nodes(nodes, &bulk_store_clone).await
                    }
                    Err(e) => Err(e),
                }
            } else {
                Ok(vec![HtmlNode::RichText {
                    text: trimmed.to_string(),
                    alignment: None,
                }])
            }
        }
        Value::Array(arr) => {
            let mut all_nodes = Vec::with_capacity(arr.len());
            for item_val in arr {
                all_nodes
                    .extend(parse_value_to_html_nodes(item_val, page_id, lang, bulk_store).await?);
            }
            Ok(all_nodes)
        }
        Value::Object(obj) => {
            log(
                LogLevel::Warning,
                &format!(
                    "Unexpected object during HTML node parsing [{} / {}]: {:?}",
                    lang, page_id, obj
                ),
            );
            Ok(vec![])
        }
        Value::Null => Ok(vec![]),
        _ => Ok(vec![]),
    }
}

async fn parse_materials_value(
    value: &Value,
    page_id: EntryId,
    lang: &str,
    bulk_store: &Arc<BulkStore>,
) -> AppResult<Vec<HtmlNode>> {
    let mut material_list: Vec<serde_json::Map<String, Value>> = Vec::new();
    let mut processed_value = false;

    match value {
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.starts_with("$[") && trimmed.ends_with("]$") {
                processed_value = true;
                let inner_json_str = trimmed.get(2..trimmed.len() - 2).unwrap_or("").trim();
                if !inner_json_str.is_empty() {
                    match from_str::<Vec<serde_json::Map<String, Value>>>(inner_json_str) {
                        Ok(list) => material_list.extend(list),
                        Err(_) => match from_str::<serde_json::Map<String, Value>>(inner_json_str) {
                            Ok(map) => material_list.push(map),
                            Err(e_inner) => log(LogLevel::Warning, &format!("Failed to parse inner material JSON string '{}' as Vec or Map: {}", inner_json_str, e_inner)),
                        },
                    }
                }
            }
        }
        Value::Array(arr) => {
            let mut successfully_parsed_from_array = false;
            for item_val in arr {
                if let Value::String(s_inner) = item_val {
                    let trimmed_inner = s_inner.trim();
                    if trimmed_inner.starts_with("$[") && trimmed_inner.ends_with("]$") {
                        processed_value = true;
                        let inner_json_str = trimmed_inner
                            .get(2..trimmed_inner.len() - 2)
                            .unwrap_or("")
                            .trim();
                        if !inner_json_str.is_empty() {
                            match from_str::<Vec<serde_json::Map<String, Value>>>(inner_json_str) {
                                Ok(list) => {
                                    material_list.extend(list);
                                    successfully_parsed_from_array = true;
                                }
                                Err(_) => match from_str::<serde_json::Map<String, Value>>(inner_json_str) {
                                    Ok(map) => {
                                        material_list.push(map);
                                        successfully_parsed_from_array = true;
                                    }
                                    Err(e_inner) => log(LogLevel::Warning, &format!("Failed to parse inner material JSON string '{}' from array element: {}", inner_json_str, e_inner)),
                                },
                            }
                        }
                    }
                } else if let Value::Object(obj) = item_val {
                    if obj.contains_key("ep_id") {
                        material_list.push(obj.clone());
                        successfully_parsed_from_array = true;
                        processed_value = true;
                    }
                }
            }
            if !successfully_parsed_from_array {
                processed_value = false;
            }
        }
        Value::Object(obj) => {
            if obj.contains_key("ep_id") {
                material_list.push(obj.clone());
                processed_value = true;
            } else {
                processed_value = false;
            }
        }
        _ => {
            processed_value = true;
        }
    }

    if !processed_value && material_list.is_empty() {
        if let Ok(list) = from_value::<Vec<serde_json::Map<String, Value>>>(value.clone()) {
            material_list = list;
        } else if let Ok(map) = from_value::<serde_json::Map<String, Value>>(value.clone()) {
            if map.contains_key("ep_id") {
                material_list.push(map);
            }
        } else {
            let is_simple_or_empty = value.is_null()
                || value.is_boolean()
                || value.is_number()
                || value.as_array().map_or(false, |a| a.is_empty())
                || value.as_object().map_or(false, |o| o.is_empty())
                || value.as_str().map_or(false, |s| {
                    s.is_empty() || (s.trim().starts_with('<') && s.trim().ends_with('>'))
                });

            if !is_simple_or_empty {
                log(
                    LogLevel::Warning,
                    &format!(
                        "Failed fallback material parse for value: {:?} on page {}/{}",
                        value, lang, page_id
                    ),
                );
            }
        }
    }

    if material_list.is_empty() {
        Ok(vec![])
    } else {
        process_material_list(material_list, bulk_store).await
    }
}

async fn process_material_list(
    material_list: Vec<serde_json::Map<String, Value>>,
    bulk_store: &Arc<BulkStore>,
) -> AppResult<Vec<HtmlNode>> {
    let mut processed_mats = Vec::with_capacity(material_list.len());
    for mat_map in material_list {
        if let Some(ep_id_val) = mat_map.get("ep_id") {
            if let Some(ep_id) = crate::transform::util::parse_value_as_optional_i64(ep_id_val) {
                if ep_id > 0 {
                    let amount = mat_map.get("amount").and_then(Value::as_i64).unwrap_or(0);
                    let name_from_map = mat_map
                        .get("name")
                        .or_else(|| mat_map.get("nickname"))
                        .and_then(Value::as_str);
                    let icon_from_map = mat_map
                        .get("icon")
                        .or_else(|| mat_map.get("icon_url"))
                        .or_else(|| mat_map.get("img"))
                        .and_then(Value::as_str);
                    let desc_from_map = mat_map.get("desc").and_then(Value::as_str);
                    let menu_id_from_map = mat_map
                        .get("menu_id")
                        .and_then(crate::transform::util::parse_value_as_optional_i64);

                    let name = bulk_store
                        .get_name(ep_id)
                        .filter(|s| !s.is_empty())
                        .map(String::from)
                        .or_else(|| name_from_map.map(String::from))
                        .unwrap_or_default();

                    let icon_url = bulk_store
                        .get_icon(ep_id)
                        .filter(|s| !s.is_empty())
                        .map(String::from)
                        .or_else(|| icon_from_map.map(String::from))
                        .unwrap_or_default();

                    let desc = bulk_store
                        .get_desc(ep_id)
                        .map(String::from)
                        .or_else(|| desc_from_map.map(String::from));

                    let display_style = mat_map
                        .get("display_style")
                        .and_then(Value::as_str)
                        .map(String::from)
                        .unwrap_or_else(html::default_display_style);

                    processed_mats.push(HtmlNode::CustomEntry {
                        ep_id,
                        name,
                        desc,
                        icon_url,
                        amount,
                        display_style,
                        menu_id: menu_id_from_map,
                    });
                }
            }
        }
    }
    Ok(processed_mats)
}

#[async_recursion]
async fn transform_base_info_list(
    items: Vec<model::ApiBaseInfoItem>,
    page_id: EntryId,
    lang: &str,
    bulk_store: &Arc<BulkStore>,
) -> AppResult<Vec<OutputBaseInfoItem>> {
    let mut results = Vec::with_capacity(items.len());
    for item in items {
        let nodes = if item.is_material == Some(true) {
            parse_materials_value(&item.value, page_id, lang, bulk_store).await?
        } else {
            parse_value_to_html_nodes(&item.value, page_id, lang, bulk_store).await?
        };
        if !item.key.is_empty() || !nodes.is_empty() {
            results.push(OutputBaseInfoItem {
                key: item.key,
                value: if nodes.is_empty() { None } else { Some(nodes) },
                is_material: item.is_material,
            });
        }
    }
    Ok(results)
}

#[async_recursion]
async fn transform_ascension_list(
    items: Vec<model::ApiAscensionItem>,
    page_id: EntryId,
    lang: &str,
    bulk_store: &Arc<BulkStore>,
) -> AppResult<Vec<OutputAscensionItem>> {
    let mut results = Vec::with_capacity(items.len());
    for item in items {
        let materials = parse_materials_value(&item.materials, page_id, lang, bulk_store).await?;
        if !item.combat_list.is_null() || !materials.is_empty() || !item.key.is_empty() {
            results.push(OutputAscensionItem {
                key: item.key,
                combat_stats: item.combat_list.clone(),
                materials: if materials.is_empty() {
                    None
                } else {
                    Some(materials)
                },
            });
        }
    }
    Ok(results)
}

#[async_recursion]
async fn transform_talent_list(
    items: Vec<model::ApiTalentItem>,
    page_id: EntryId,
    lang: &str,
    bulk_store: &Arc<BulkStore>,
) -> AppResult<Vec<OutputTalentItem>> {
    let mut results = Vec::with_capacity(items.len());
    for item in items {
        let desc_nodes = parse_value_to_html_nodes(&item.desc, page_id, lang, bulk_store).await?;

        let mut processed_levels: Option<Vec<Option<Vec<HtmlNode>>>> = None;
        if let Value::Array(levels_val) = &item.materials {
            let mut level_results = Vec::with_capacity(levels_val.len());
            for level_val in levels_val {
                if !level_val.is_null() && !level_val.as_array().map_or(false, |a| a.is_empty()) {
                    let nodes = parse_materials_value(level_val, page_id, lang, bulk_store).await?;
                    level_results.push(if nodes.is_empty() { None } else { Some(nodes) });
                } else {
                    level_results.push(None);
                }
            }
            if !level_results.is_empty() || !levels_val.is_empty() {
                processed_levels = Some(level_results);
            }
        } else if !item.materials.is_null() {
            let single_level_nodes =
                parse_materials_value(&item.materials, page_id, lang, bulk_store).await?;
            if !single_level_nodes.is_empty() {
                processed_levels = Some(vec![Some(single_level_nodes)]);
            }
        }

        if !desc_nodes.is_empty()
            || !item.attributes.is_null()
            || processed_levels.is_some()
            || item.title.is_some()
            || item.icon_url.is_some()
        {
            results.push(OutputTalentItem {
                key: item.key,
                title: item.title.unwrap_or_default(),
                icon_url: item.icon_url.unwrap_or_default(),
                desc: desc_nodes,
                attributes: item.attributes.clone(),
                materials: processed_levels,
                talent_img: item.talent_img,
            });
        }
    }
    Ok(results)
}

#[async_recursion]
async fn transform_summary_list(
    items: Vec<model::ApiSummaryItem>,
    page_id: EntryId,
    lang: &str,
    bulk_store: &Arc<BulkStore>,
) -> AppResult<Vec<OutputSummaryItem>> {
    let mut results = Vec::with_capacity(items.len());
    for item in items {
        let desc_nodes = parse_value_to_html_nodes(&item.desc, page_id, lang, bulk_store).await?;
        if !desc_nodes.is_empty() || !item.name.is_empty() || !item.icon_url.is_empty() {
            results.push(OutputSummaryItem {
                icon_url: item.icon_url,
                name: item.name,
                desc: desc_nodes,
            });
        }
    }
    Ok(results)
}

#[async_recursion]
async fn transform_story_list(
    items: Vec<model::ApiStoryItem>,
    page_id: EntryId,
    lang: &str,
    bulk_store: &Arc<BulkStore>,
) -> AppResult<Vec<OutputStoryItem>> {
    let mut results = Vec::with_capacity(items.len());
    for item in items {
        let desc_nodes = parse_value_to_html_nodes(&item.desc, page_id, lang, bulk_store).await?;
        if !desc_nodes.is_empty() || !item.title.is_empty() {
            results.push(OutputStoryItem {
                title: item.title,
                desc: desc_nodes,
            });
        }
    }
    Ok(results)
}

fn transform_voice_list(items: Vec<model::ApiVoiceItem>) -> AppResult<Vec<OutputVoiceItem>> {
    Ok(items.into_iter().filter_map(transform_voice_item).collect())
}

fn transform_voice_item(item: model::ApiVoiceItem) -> Option<OutputVoiceItem> {
    let mut output_audios = vec![];
    if let Some(Value::Array(audios_val)) = item.audios {
        for audio_v in audios_val {
            match from_value::<AudioInfo>(audio_v) {
                Ok(audio_info) => {
                    output_audios.push(audio_info);
                }
                Err(e) => {
                    log(
                        LogLevel::Warning,
                        &format!("Failed to parse voice audio info: {}", e),
                    );
                }
            }
        }
    }
    if item.title.is_empty() && item.desc.is_empty() && output_audios.is_empty() {
        None
    } else {
        Some(OutputVoiceItem {
            title: item.title,
            desc: item.desc,
            audios: output_audios,
        })
    }
}

#[async_recursion]
async fn transform_gallery_list(
    items: Vec<model::ApiGalleryCharacterItem>,
    page_id: EntryId,
    lang: &str,
    bulk_store: &Arc<BulkStore>,
) -> AppResult<Vec<OutputGalleryCharacterItem>> {
    let mut results = Vec::with_capacity(items.len());
    for item in items {
        let desc_nodes =
            parse_value_to_html_nodes(&item.img_desc, page_id, lang, bulk_store).await?;
        if !item.img.is_empty() || !item.key.is_empty() {
            results.push(OutputGalleryCharacterItem {
                key: item.key,
                img: item.img,
                img_desc: desc_nodes,
            });
        }
    }
    Ok(results)
}

#[async_recursion]
async fn transform_gallery_character_wrapper(
    wrapper: model::GalleryCharacterWrapper,
    page_id: EntryId,
    lang: &str,
    bulk_store: &Arc<BulkStore>,
) -> AppResult<Vec<OutputGalleryCharacterItem>> {
    let mut transformed_list =
        transform_gallery_list(wrapper.list, page_id, lang, bulk_store).await?;

    if let Some(pic_url) = wrapper.pic {
        if !pic_url.is_empty() {
            let character_card_key = match lang {
                "id-id" => "Kartu Karakter",

                _ => "Character Card",
            };

            let pic_item = OutputGalleryCharacterItem {
                key: character_card_key.to_string(),
                img: pic_url,
                img_desc: vec![],
            };

            transformed_list.insert(0, pic_item);
        }
    }

    Ok(transformed_list)
}

#[async_recursion]
async fn transform_artifact_map(
    item_map: HashMap<String, model::ApiArtifactListItem>,
    page_id: EntryId,
    lang: &str,
    bulk_store: &Arc<BulkStore>,
) -> AppResult<HashMap<String, OutputArtifactListItem>> {
    let mut results = HashMap::with_capacity(item_map.len());
    for (key, item) in item_map {
        let desc_nodes = parse_value_to_html_nodes(&item.desc, page_id, lang, bulk_store).await?;
        if !desc_nodes.is_empty() || !item.icon_url.is_empty() || item.title.is_some() {
            results.insert(
                key.clone(),
                OutputArtifactListItem {
                    title: item.title,
                    position: item.position,
                    desc: desc_nodes,
                    icon_url: item.icon_url,
                },
            );
        }
    }
    Ok(results)
}
