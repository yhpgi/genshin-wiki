use crate::api;
use crate::config;
use crate::model::{
    common::EntryId,
    output::{FilterValue, OutputListFile, OutputNavMenuItem},
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

pub fn transform_nav_item(entry: &api::model::ApiNavEntry) -> Option<OutputNavMenuItem> {
    match (&entry.menu, &entry.name) {
        (Some(menu), Some(name)) if !name.trim().is_empty() => Some(OutputNavMenuItem {
            menu_id: menu.menu_id,
            name: name.trim().to_string(),
            icon_url: entry.icon_url.as_deref().unwrap_or("").trim().to_string(),
        }),
        _ => None,
    }
}

pub fn to_camel_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalize_next = false;
    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

pub fn process_filters_value(raw_filters_val: &Value) -> HashMap<String, FilterValue> {
    let mut processed_map = HashMap::new();
    if let Value::Object(raw_filters) = raw_filters_val {
        for &snake_key in config::LIST_FILTER_FIELDS.iter() {
            if let Some(field_data) = raw_filters.get(snake_key) {
                let camel_key = to_camel_case(snake_key);
                let is_rarity_key =
                    snake_key == config::KEY_CHAR_RARITY || snake_key == config::KEY_WEAPON_RARITY;

                let extracted_string_values = extract_filter_field_values(field_data);

                if !extracted_string_values.is_empty() {
                    if is_rarity_key {
                        if let Some(rarity_int) = extracted_string_values
                            .iter()
                            .find_map(|s| s.chars().next().and_then(|c| c.to_digit(10)))
                        {
                            processed_map
                                .insert(camel_key, FilterValue::Integer(rarity_int as i64));
                        } else {
                            if let Some(first_val) = extracted_string_values.iter().next() {
                                processed_map
                                    .insert(camel_key, FilterValue::Single(first_val.clone()));
                            }
                        }
                    } else {
                        let mut sorted_values: Vec<String> =
                            extracted_string_values.into_iter().collect();
                        sorted_values.sort_unstable();

                        if config::MULTI_VALUE_FILTER_FIELDS.contains(snake_key) {
                            processed_map.insert(camel_key, FilterValue::Multiple(sorted_values));
                        } else if let Some(first_val) = sorted_values.into_iter().next() {
                            processed_map.insert(camel_key, FilterValue::Single(first_val));
                        }
                    }
                }
            }
        }
    }
    processed_map
}

fn extract_filter_field_values(field_data: &Value) -> HashSet<String> {
    let mut values_set: HashSet<String> = HashSet::new();

    let target_values = field_data
        .get("values")
        .or_else(|| field_data.get("value"))
        .or_else(|| {
            field_data
                .get("value_types")
                .and_then(|vt| vt.get(0))
                .and_then(|ft| ft.get("value"))
        });

    match target_values {
        Some(Value::Array(vals)) => {
            for v in vals {
                if let Some(s) = value_to_string(v) {
                    values_set.insert(s);
                }
            }
        }
        Some(single_val) => {
            if let Some(s) = value_to_string(single_val) {
                values_set.insert(s);
            }
        }
        None => {
            if let Some(s) = value_to_string(field_data) {
                values_set.insert(s);
            }
        }
    }
    values_set
}

#[inline]
fn value_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

pub fn build_metadata_map(
    output_lists: &[OutputListFile],
) -> HashMap<EntryId, HashMap<&'static str, String>> {
    let mut metadata_map = HashMap::new();

    let keys_to_extract = [
        config::KEY_CHAR_VISION,
        config::KEY_CHAR_RARITY,
        config::KEY_WEAPON_RARITY,
    ];

    for list_file in output_lists {
        for item in &list_file.list {
            let entry_metadata = metadata_map.entry(item.id).or_insert_with(HashMap::new);
            for &snake_key in &keys_to_extract {
                let camel_key = to_camel_case(snake_key);
                if let Some(filter_val) = item.filter_values.get(&camel_key) {
                    let value_str = match filter_val {
                        FilterValue::Single(s) if !s.is_empty() => Some(s.clone()),
                        FilterValue::Multiple(v) => v.iter().find(|s| !s.is_empty()).cloned(),
                        FilterValue::Integer(i) => Some(i.to_string()),
                        _ => None,
                    };
                    if let Some(val) = value_str {
                        entry_metadata.insert(snake_key, val);
                    }
                }
            }
        }
    }
    metadata_map
}
