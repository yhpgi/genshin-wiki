pub mod bulk;
pub mod calendar;
pub mod common;
pub mod detail;
pub mod html_parser;
pub mod list;
pub mod util;

use crate::api::model::{self, ApiComponentData};
use crate::core::data_store::{RawData, TransformedData};
use crate::error::{AppError, AppResult};
use crate::logging::{log, LogLevel};
use crate::model as output_model;
use crate::model::common::{EntryId, MenuId};
use crate::transform::bulk::BulkStore;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;
use tokio::task::JoinSet;

pub async fn transform_all_data(
    raw_data: Arc<RawData>,
    all_bulk_stores: HashMap<String, BulkStore>,
    languages: &[String],
) -> AppResult<TransformedData> {
    log(LogLevel::Info, "--- Transforming all fetched data ---");
    let start_time = Instant::now();
    let mut transformed = TransformedData::default();

    let nav_lookup_maps: HashMap<String, Arc<HashMap<MenuId, String>>> = raw_data
        .navigation
        .iter()
        .map(|(lang, entries)| {
            let lookup = entries
                .iter()
                .filter_map(|entry| match (&entry.menu, &entry.name) {
                    (Some(menu), Some(name)) if !name.is_empty() => {
                        Some((menu.menu_id, name.trim().to_string()))
                    }
                    _ => None,
                })
                .collect::<HashMap<_, _>>();
            (lang.clone(), Arc::new(lookup))
        })
        .collect();

    let mut transformation_tasks: JoinSet<Result<LangTransformResult, AppError>> = JoinSet::new();

    let all_bulk_stores_arc = Arc::new(all_bulk_stores);
    let raw_data_arc = raw_data;

    for lang in languages {
        let lang_clone = lang.clone();
        let bulk_store = all_bulk_stores_arc.get(lang).cloned().unwrap_or_default();
        let nav_lookup = nav_lookup_maps
            .get(lang)
            .cloned()
            .unwrap_or_else(|| Arc::new(HashMap::new()));
        let raw_data_for_lang = Arc::clone(&raw_data_arc);

        transformation_tasks.spawn(async move {
            transform_language_data(&lang_clone, raw_data_for_lang, bulk_store, nav_lookup).await
        });
    }

    while let Some(result) = transformation_tasks.join_next().await {
        match result {
            Ok(Ok(lang_result)) => {
                transformed
                    .navigation
                    .insert(lang_result.lang.clone(), lang_result.navigation);
                transformed
                    .lists
                    .insert(lang_result.lang.clone(), lang_result.lists);
                transformed
                    .details
                    .insert(lang_result.lang.clone(), lang_result.details);
                if let Some(cal) = lang_result.calendar {
                    transformed.calendars.insert(lang_result.lang.clone(), cal);
                }
            }
            Ok(Err(e)) => {
                log(
                    LogLevel::Error,
                    &format!("Language transformation task failed: {:?}", e),
                );
            }
            Err(e) => {
                log(
                    LogLevel::Error,
                    &format!("Language transformation task panicked: {}", e),
                );
            }
        }
    }

    log(
        LogLevel::Success,
        &format!(
            "--- Data transformation complete | Elapsed: {:?} ---",
            start_time.elapsed()
        ),
    );
    Ok(transformed)
}

struct LangTransformResult {
    lang: String,
    navigation: Vec<output_model::output::OutputNavMenuItem>,
    lists: Vec<output_model::output::OutputListFile>,
    details: Vec<output_model::output::OutputDetailPage>,
    calendar: Option<output_model::output::OutputCalendarFile>,
}

async fn transform_language_data(
    lang: &str,
    raw_data: Arc<RawData>,
    bulk_store: BulkStore,
    nav_lookup: Arc<HashMap<MenuId, String>>,
) -> AppResult<LangTransformResult> {
    let bulk_store_arc = Arc::new(bulk_store);

    let output_nav = raw_data
        .navigation
        .get(lang)
        .map_or_else(Vec::new, |entries| {
            entries
                .iter()
                .filter_map(common::transform_nav_item)
                .collect()
        });

    let mut output_lists = Vec::new();
    if let Some(lang_list_map) = raw_data.lists.get(lang) {
        for (&menu_id, items) in lang_list_map {
            let menu_name = nav_lookup
                .get(&menu_id)
                .cloned()
                .unwrap_or_else(|| format!("Menu {}", menu_id));

            if let Some(lf) =
                list::transform_list_file(items.clone(), &bulk_store_arc, lang, menu_id, menu_name)
            {
                output_lists.push(lf);
            }
        }
    }

    let mut detail_tasks = JoinSet::new();
    let mut output_details = Vec::new();
    if let Some(detail_pages) = raw_data.details.get(lang) {
        output_details.reserve(detail_pages.len());
        for detail_page in detail_pages.clone() {
            let bulk_store_c = bulk_store_arc.clone();
            let lang_c = lang.to_string();
            detail_tasks.spawn(async move {
                detail::transform_detail_page(detail_page, bulk_store_c, &lang_c).await
            });
        }
        while let Some(result) = detail_tasks.join_next().await {
            match result {
                Ok(Ok(Some(od))) => output_details.push(od),
                Ok(Ok(None)) => {}
                Ok(Err(e)) => log(
                    LogLevel::Warning,
                    &format!("Detail transform error [{}]: {:?}", lang, e),
                ),
                Err(e) => log(
                    LogLevel::Error,
                    &format!("Detail transform task panicked [{}]: {}", lang, e),
                ),
            }
        }
    }
    output_details.sort_unstable_by_key(|d| d.id);

    let output_calendar = if let Some(cal_resp) = raw_data.calendars.get(lang) {
        let metadata_map = common::build_metadata_map(&output_lists);
        match calendar::transform_calendar(cal_resp.clone(), bulk_store_arc, &metadata_map, lang) {
            Ok(opt_cal) => opt_cal,
            Err(e) => {
                log(
                    LogLevel::Warning,
                    &format!("Calendar transform error [{}]: {:?}", lang, e),
                );
                None
            }
        }
    } else {
        None
    };

    Ok(LangTransformResult {
        lang: lang.to_string(),
        navigation: output_nav,
        lists: output_lists,
        details: output_details,
        calendar: output_calendar,
    })
}

pub fn collect_all_ids(raw_data: &RawData) -> HashMap<String, HashSet<EntryId>> {
    let mut all_ids_map: HashMap<String, HashSet<EntryId>> = HashMap::new();

    let mut collector = IdCollector::new();

    for (lang, lists_map) in &raw_data.lists {
        let lang_ids = all_ids_map.entry(lang.clone()).or_default();
        for items in lists_map.values() {
            for item in items {
                collector.collect_from_list_item(item, lang_ids);
            }
        }
    }

    for (lang, details) in &raw_data.details {
        let lang_ids = all_ids_map.entry(lang.clone()).or_default();
        for detail in details {
            collector.collect_from_detail_page(detail, lang_ids);
        }
    }

    for (lang, calendar) in &raw_data.calendars {
        let lang_ids = all_ids_map.entry(lang.clone()).or_default();
        for item in calendar.calendar.iter().chain(calendar.op.iter()) {
            collector.collect_from_value(item, lang_ids);
        }
    }

    all_ids_map
}

struct IdCollector;

impl IdCollector {
    fn new() -> Self {
        Self
    }

    fn collect_from_list_item(&mut self, item: &model::ApiListItem, ids: &mut HashSet<EntryId>) {
        if item.entry_page_id > 0 {
            ids.insert(item.entry_page_id);
        }
        if let Some(df) = &item.display_field {
            self.collect_from_value(df, ids);
        }
        self.collect_from_value(&item.filter_values, ids);
    }

    fn collect_from_detail_page(
        &mut self,
        detail: &model::ApiDetailPage,
        ids: &mut HashSet<EntryId>,
    ) {
        if let Some(id) = detail.id {
            if id > 0 {
                ids.insert(id);
            }
        }
        self.collect_from_value(&detail.filter_values, ids);
        for module in &detail.modules {
            self.collect_from_module(module, ids);
        }
    }

    fn collect_from_module(&mut self, module: &model::ApiModule, ids: &mut HashSet<EntryId>) {
        for component in &module.components {
            self.collect_from_component(component, ids);
        }

        if !module.modules.is_empty() {
            for nested_module in &module.modules {
                self.collect_from_module(nested_module, ids);
            }
        }
    }

    fn collect_from_component(
        &mut self,
        component: &model::ApiComponent,
        ids: &mut HashSet<EntryId>,
    ) {
        match &component.typed_data {
            ApiComponentData::BaseInfoList(items) => {
                for item in items {
                    self.collect_from_value(&item.value, ids);
                }
            }
            ApiComponentData::AscensionList(items) => {
                for item in items {
                    self.collect_from_value(&item.materials, ids);
                }
            }
            ApiComponentData::TalentList(items) => {
                for item in items {
                    self.collect_from_value(&item.desc, ids);
                    self.collect_from_value(&item.materials, ids);
                }
            }
            ApiComponentData::SummaryList(items) => {
                for item in items {
                    self.collect_from_value(&item.desc, ids);
                }
            }
            ApiComponentData::StoryList(items) => {
                for item in items {
                    self.collect_from_value(&item.desc, ids);
                }
            }
            ApiComponentData::BodyList(items) => {
                for item in items {
                    self.collect_from_value(&item.content, ids);
                }
            }
            ApiComponentData::GalleryCharacterWrapperData(wrapper) => {
                for item in &wrapper.list {
                    self.collect_from_value(&item.img_desc, ids);
                }
            }
            ApiComponentData::GalleryCharacterList(items) => {
                for item in items {
                    self.collect_from_value(&item.img_desc, ids);
                }
            }
            ApiComponentData::ArtifactMap(map) => {
                for item in map.values() {
                    self.collect_from_value(&item.desc, ids);
                }
            }
            ApiComponentData::Customize(data_val) => {
                self.collect_from_value(data_val, ids);
            }
            ApiComponentData::TextualResearchList(items) => {
                for item in items {
                    self.collect_from_value(&item.desc, ids);
                }
            }
            ApiComponentData::Timeline(data) => {
                for event in &data.list {
                    for module_content in &event.modules {
                        self.collect_from_value(&Value::String(module_content.desc.clone()), ids);
                    }
                }
            }
            ApiComponentData::VideoCollection(data_val) => {
                self.collect_from_value(data_val, ids);
            }
            ApiComponentData::DropMaterial(data) => {
                for mat_str in &data.list {
                    self.collect_from_value(&Value::String(mat_str.clone()), ids);
                }
            }
            ApiComponentData::Unknown(v) => self.collect_from_value(v, ids),
            ApiComponentData::VoiceList(_)
            | ApiComponentData::ReliquarySetEffect(_)
            | ApiComponentData::Map(_)
            | ApiComponentData::Tcg(_) => {}
        }
    }

    fn collect_from_value(&mut self, data: &Value, ids: &mut HashSet<EntryId>) {
        match data {
            Value::Object(map) => {
                if let Some(id) = map
                    .get("ep_id")
                    .or_else(|| map.get("entry_page_id"))
                    .and_then(util::parse_value_as_optional_i64)
                {
                    if id > 0 {
                        ids.insert(id);
                    }
                }
                if let Some(id) = map
                    .get("post_id")
                    .and_then(util::parse_value_as_optional_i64)
                {
                    if id > 0 {
                        ids.insert(id);
                    }
                }
                if let Some(id) = map.get("id").and_then(util::parse_value_as_optional_i64) {
                    if id > 0
                        && (map.contains_key("key")
                            || map.contains_key("title")
                            || map.contains_key("img"))
                    {}
                }
                for value in map.values() {
                    self.collect_from_value(value, ids);
                }
            }
            Value::Array(vec) => {
                for item in vec {
                    self.collect_from_value(item, ids);
                }
            }
            Value::String(s) => {
                let trimmed = s.trim();
                if (trimmed.starts_with("$[") && trimmed.ends_with("]$"))
                    || (trimmed.starts_with("\"$[") && trimmed.ends_with("]$\""))
                {
                    let json_str = if trimmed.starts_with("\"$[") {
                        trimmed
                            .get(3..trimmed.len().saturating_sub(3))
                            .unwrap_or("")
                    } else {
                        trimmed
                            .get(2..trimmed.len().saturating_sub(2))
                            .unwrap_or("")
                    };

                    if !json_str.is_empty() {
                        match serde_json::from_str::<Value>(json_str) {
                            Ok(parsed_value) => self.collect_from_value(&parsed_value, ids),
                            Err(e) => {
                                if json_str.starts_with('{') && json_str.ends_with('}') {
                                    match serde_json::from_str::<Value>(&format!("[{}]", json_str)) {
                                        Ok(wrapped_value) => self.collect_from_value(&wrapped_value, ids),
                                        Err(e2) => log(LogLevel::Warning, &format!("Failed secondary parse of material string '{}': {}", json_str, e2)),
                                    }
                                } else {
                                    log(
                                        LogLevel::Warning,
                                        &format!(
                                            "Failed to parse material string '{}': {}",
                                            json_str, e
                                        ),
                                    );
                                }
                            }
                        }
                    }
                } else if (trimmed.starts_with('{') && trimmed.ends_with('}'))
                    || (trimmed.starts_with('[') && trimmed.ends_with(']'))
                {
                    match serde_json::from_str::<Value>(trimmed) {
                        Ok(parsed_value) => self.collect_from_value(&parsed_value, ids),
                        Err(e) => log(
                            LogLevel::Warning,
                            &format!("Failed to parse direct JSON string '{}': {}", trimmed, e),
                        ),
                    }
                }
            }
            _ => {}
        }
    }
}
