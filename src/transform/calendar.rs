use crate::api;
use crate::config;
use crate::error::AppResult;
use crate::model::common::EntryId;
use crate::model::output::{
    OutputCalendarAbstract, OutputCalendarFile, OutputCalendarItem, OutputCalendarOpItem,
};
use crate::transform::bulk::BulkStore;
use crate::transform::util;
use chrono::Utc;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

pub fn transform_calendar(
    raw: api::model::ApiCalendarResponse,
    bulk_store: Arc<BulkStore>,

    metadata_map: &HashMap<EntryId, HashMap<&'static str, String>>,
    lang: &str,
) -> AppResult<Option<OutputCalendarFile>> {
    let mut output_calendar_items = Vec::new();
    let mut output_op_items = Vec::new();

    for item_val in raw.calendar {
        if let Value::Object(map) = &item_val {
            let character_abstracts =
                process_abstracts_value(map.get("character_abstracts"), &bulk_store, metadata_map);
            let material_abstracts =
                process_abstracts_value(map.get("material_abstracts"), &bulk_store, metadata_map);
            let ep_abstracts =
                process_abstracts_value(map.get("ep_abstracts"), &bulk_store, metadata_map);

            let drop_day_value = map
                .get("drop_day")
                .filter(|v| !v.is_null() && !v.as_array().map_or(false, |a| a.is_empty()))
                .cloned();
            let break_type = map
                .get("break_type")
                .and_then(util::parse_value_as_optional_i64);
            let obtain_method = map
                .get("obtain_method")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(String::from);
            let has_abstracts = !character_abstracts.is_empty()
                || !material_abstracts.is_empty()
                || !ep_abstracts.is_empty();

            let item = OutputCalendarItem {
                drop_day: drop_day_value,
                break_type,
                obtain_method,
                character_abstracts,
                material_abstracts,
                ep_abstracts,
            };

            if item.drop_day.is_some()
                || item.break_type.is_some()
                || item.obtain_method.is_some()
                || has_abstracts
            {
                output_calendar_items.push(item);
            }
        }
    }

    for item_val in raw.op {
        if let Value::Object(map) = &item_val {
            let ep_abstracts =
                process_abstracts_value(map.get("ep_abstracts"), &bulk_store, metadata_map);
            let start_time = util::format_calendar_date_value(map.get("start_time"));
            let end_time = util::format_calendar_date_value(map.get("end_time"));
            let text = map
                .get("text")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(String::from);
            let title = map
                .get("title")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(String::from);
            let is_birth = map.get("is_birth").and_then(Value::as_bool);

            let item = OutputCalendarOpItem {
                is_birth,
                text,
                title,
                start_time,
                end_time,
                ep_abstracts: ep_abstracts.clone(),
            };

            if item.is_birth.is_some()
                || item.text.is_some()
                || item.title.is_some()
                || item.start_time.is_some()
                || item.end_time.is_some()
                || !item.ep_abstracts.is_empty()
            {
                output_op_items.push(item);
            }
        }
    }

    output_op_items.sort_by(|a, b| a.start_time.cmp(&b.start_time));

    if output_calendar_items.is_empty() && output_op_items.is_empty() {
        Ok(None)
    } else {
        Ok(Some(OutputCalendarFile {
            version: Utc::now(),
            language: lang.to_string(),
            calendar: output_calendar_items,
            op: output_op_items,
        }))
    }
}

fn process_abstracts_value(
    abstract_val: Option<&Value>,
    bulk_store: &Arc<BulkStore>,
    metadata_map: &HashMap<EntryId, HashMap<&'static str, String>>,
) -> Vec<OutputCalendarAbstract> {
    let mut simplified_abstracts = Vec::new();

    if let Some(Value::Array(list)) = abstract_val {
        for val in list {
            if let Value::Object(map) = val {
                if let Some(id) = util::parse_value_as_optional_i64(
                    map.get("entry_page_id")
                        .or_else(|| map.get("id"))
                        .unwrap_or(&Value::Null),
                ) {
                    if id > 0 {
                        let name = bulk_store.get_name(id).unwrap_or_default().to_string();
                        let icon_url = bulk_store.get_icon(id).unwrap_or_default().to_string();
                        let desc = bulk_store.get_desc(id).map(String::from);

                        let item_metadata = metadata_map.get(&id);

                        let vision = item_metadata
                            .and_then(|m| m.get(config::KEY_CHAR_VISION))
                            .cloned();

                        let char_rarity = item_metadata
                            .and_then(|m| m.get(config::KEY_CHAR_RARITY))
                            .and_then(|s| s.parse::<i64>().ok());

                        let weapon_rarity = item_metadata
                            .and_then(|m| m.get(config::KEY_WEAPON_RARITY))
                            .and_then(|s| s.parse::<i64>().ok());

                        if !name.is_empty() || !icon_url.is_empty() {
                            simplified_abstracts.push(OutputCalendarAbstract {
                                id,
                                name,
                                icon_url,
                                desc,
                                character_vision: vision,
                                character_rarity: char_rarity,
                                weapon_rarity: weapon_rarity,
                            });
                        }
                    }
                }
            }
        }
    }
    simplified_abstracts.sort_unstable_by_key(|a| a.id);
    simplified_abstracts
}
