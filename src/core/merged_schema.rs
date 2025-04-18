use crate::config;
use crate::core::data_store::TransformedData;
use crate::error::AppResult;
use crate::io;
use crate::model::common::MenuId;
use crate::model::output::{
    ComponentData, FilterValue, OutputCalendarAbstract, OutputCalendarFile, OutputCalendarItem,
    OutputCalendarOpItem, OutputDetailPage, OutputGalleryCharacterItem, OutputListFile,
    OutputListItem, OutputNavMenuItem,
};
use crate::transform::common::to_camel_case;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use tokio::fs;

pub async fn create_merged_schema_files(
    transformed_data: Arc<TransformedData>,
    output_dir: &Path,
) -> AppResult<()> {
    let merged_dir = output_dir.join("merged");
    fs::create_dir_all(&merged_dir).await?;

    let nav_example = generate_merged_nav_example(&transformed_data);
    let path_nav = merged_dir.join("navigation.json");

    io::save_json(path_nav, nav_example, "Merged Nav Schema".to_string()).await?;

    let list_example = generate_merged_list_example(&transformed_data);
    let path_list = merged_dir.join("list.json");

    io::save_json(path_list, list_example, "Merged List Schema".to_string()).await?;

    let detail_example = generate_merged_detail_example(&transformed_data);
    let path_detail = merged_dir.join("detail.json");

    io::save_json(
        path_detail,
        detail_example,
        "Merged Detail Schema".to_string(),
    )
    .await?;

    let calendar_example = generate_merged_calendar_example(&transformed_data);
    let path_calendar = merged_dir.join("calendar.json");

    io::save_json(
        path_calendar,
        calendar_example,
        "Merged Calendar Schema".to_string(),
    )
    .await?;

    Ok(())
}

fn generate_merged_nav_example(transformed_data: &TransformedData) -> Vec<OutputNavMenuItem> {
    transformed_data
        .navigation
        .values()
        .find(|v| !v.is_empty())
        .cloned()
        .unwrap_or_else(|| {
            vec![OutputNavMenuItem {
                menu_id: 0,
                name: "Default Nav".to_string(),
                icon_url: "default.png".to_string(),
            }]
        })
}

fn generate_merged_list_example(transformed_data: &TransformedData) -> OutputListFile {
    let mut all_filter_keys: HashSet<String> = HashSet::new();
    let mut first_list_item: Option<OutputListItem> = None;
    let mut first_list_file_metadata: Option<(i64, String, String, MenuId)> = None;

    for (lang, lang_lists) in &transformed_data.lists {
        for list_file in lang_lists {
            if !list_file.list.is_empty() {
                if first_list_item.is_none() {
                    first_list_item = list_file.list.first().cloned();
                }
                if first_list_file_metadata.is_none() {
                    first_list_file_metadata = Some((
                        list_file.version.timestamp(),
                        lang.clone(),
                        list_file.menu_name.clone(),
                        list_file.menu_id,
                    ));
                }
            } else if first_list_file_metadata.is_none() {
                first_list_file_metadata = Some((
                    list_file.version.timestamp(),
                    lang.clone(),
                    list_file.menu_name.clone(),
                    list_file.menu_id,
                ));
            }
            for item in &list_file.list {
                for key in item.filter_values.keys() {
                    all_filter_keys.insert(key.clone());
                }
            }
        }
    }

    for &snake_key in config::LIST_FILTER_FIELDS.iter() {
        all_filter_keys.insert(to_camel_case(snake_key));
    }

    let (version_ts, lang, menu_name, menu_id) = first_list_file_metadata.unwrap_or_else(|| {
        (
            Utc::now().timestamp(),
            "default_lang".to_string(),
            "Default Menu".to_string(),
            0,
        )
    });

    let mut example_item = first_list_item.unwrap_or_else(|| OutputListItem {
        id: 0,
        name: "Default Item".to_string(),
        icon_url: "default.png".to_string(),
        desc: Some("Default description.".to_string()),
        filter_values: HashMap::new(),
    });

    let mut sorted_keys: Vec<String> = all_filter_keys.into_iter().collect();
    sorted_keys.sort_unstable();

    for key in sorted_keys {
        example_item
            .filter_values
            .entry(key.clone())
            .or_insert_with(|| {
                let is_rarity = key == to_camel_case(config::KEY_CHAR_RARITY)
                    || key == to_camel_case(config::KEY_WEAPON_RARITY);
                let is_multi = config::MULTI_VALUE_FILTER_FIELDS
                    .iter()
                    .any(|&sk| to_camel_case(sk) == key);

                if is_rarity {
                    FilterValue::Integer(0)
                } else if is_multi {
                    FilterValue::Multiple(vec![format!("Default Multi {}", key)])
                } else {
                    FilterValue::Single(format!("Default Single {}", key))
                }
            });
    }

    OutputListFile {
        version: DateTime::from_timestamp(version_ts, 0).unwrap_or_else(Utc::now),
        language: lang,
        menu_id,
        menu_name,
        total_items: 1,
        list: vec![example_item],
    }
}

fn generate_merged_detail_example(transformed_data: &TransformedData) -> OutputDetailPage {
    let mut merged_components: BTreeMap<String, ComponentData> = BTreeMap::new();
    let mut first_page_metadata: Option<OutputDetailPage> = None;

    let mut all_component_keys: HashSet<String> = transformed_data
        .details
        .values()
        .flatten()
        .flat_map(|page| page.components.keys().cloned())
        .collect();

    let default_keys_camel = [
        config::COMPONENT_BASE_INFO,
        config::COMPONENT_ASCENSION,
        config::COMPONENT_TALENT,
        config::COMPONENT_SUMMARY_LIST,
        config::COMPONENT_STORY,
        config::COMPONENT_VOICE,
        config::COMPONENT_GALLERY_CHARACTER,
        config::COMPONENT_ARTIFACT_LIST,
        config::COMPONENT_RELIQUARY_SET_EFFECT,
        config::COMPONENT_MAP,
        config::COMPONENT_CUSTOMIZE,
        config::COMPONENT_TEXTUAL_RESEARCH,
        config::COMPONENT_TIMELINE,
        config::COMPONENT_VIDEO_COLLECTION,
        config::COMPONENT_TCG,
        config::COMPONENT_DROP_MATERIAL,
        config::COMPONENT_BODY,
    ];
    for &key in default_keys_camel.iter() {
        all_component_keys.insert(to_camel_case(key));
    }

    for page in transformed_data.details.values().flatten() {
        if first_page_metadata.is_none() {
            first_page_metadata = Some(OutputDetailPage {
                id: page.id,
                name: page.name.clone(),
                desc: page.desc.clone(),
                icon_url: page.icon_url.clone(),
                header_img_url: page.header_img_url.clone(),
                filter_values: page.filter_values.clone(),
                menu_id: page.menu_id,
                menu_name: page.menu_name.clone(),
                version: page.version,
                components: HashMap::new(),
            });
        }

        for (key, data) in &page.components {
            if !merged_components.contains_key(key)
                || matches!(merged_components.get(key), Some(ComponentData::Unknown(v)) if v.is_null())
            {
                if !matches!(data, ComponentData::Unknown(v) if v.is_null()) {
                    merged_components.insert(key.clone(), data.clone());
                } else if !merged_components.contains_key(key) {
                    merged_components.insert(key.clone(), data.clone());
                }
            }
        }
    }

    let mut base_page = first_page_metadata.unwrap_or_else(|| OutputDetailPage {
        id: 0,
        name: Some("Default Name".to_string()),
        desc: Some("Default Desc".to_string()),
        icon_url: Some("default.png".to_string()),
        header_img_url: Some("default_header.png".to_string()),
        filter_values: HashMap::new(),
        menu_id: 0,
        menu_name: Some("Default Menu".to_string()),
        version: Utc::now().timestamp(),
        components: HashMap::new(),
    });

    let mut all_filter_keys_detail: HashSet<String> = transformed_data
        .details
        .values()
        .flatten()
        .flat_map(|page| page.filter_values.keys().cloned())
        .collect();

    for &snake_key in config::LIST_FILTER_FIELDS.iter() {
        all_filter_keys_detail.insert(to_camel_case(snake_key));
    }

    let mut sorted_filter_keys: Vec<String> = all_filter_keys_detail.into_iter().collect();
    sorted_filter_keys.sort_unstable();

    for key in sorted_filter_keys {
        base_page
            .filter_values
            .entry(key.clone())
            .or_insert_with(|| {
                let is_rarity = key == to_camel_case(config::KEY_CHAR_RARITY)
                    || key == to_camel_case(config::KEY_WEAPON_RARITY);
                let is_multi = config::MULTI_VALUE_FILTER_FIELDS
                    .iter()
                    .any(|&sk| to_camel_case(sk) == key);
                if is_rarity {
                    FilterValue::Integer(0)
                } else if is_multi {
                    FilterValue::Multiple(vec![format!("Default Multi {}", key)])
                } else {
                    FilterValue::Single(format!("Default Single {}", key))
                }
            });
    }

    let mut sorted_component_keys: Vec<String> = all_component_keys.into_iter().collect();
    sorted_component_keys.sort_unstable();

    for key in sorted_component_keys {
        merged_components
            .entry(key.clone())
            .or_insert_with(|| create_default_component_data(&key));
    }

    base_page.components = merged_components.into_iter().collect();
    base_page
}

fn create_default_component_data(camel_case_key: &str) -> ComponentData {
    match camel_case_key {
        "baseInfo" => ComponentData::BaseInfo(vec![Default::default()]),
        "ascension" => ComponentData::Ascension(vec![Default::default()]),
        "talent" => ComponentData::Talent(vec![Default::default()]),
        "summaryList" => ComponentData::SummaryList(vec![Default::default()]),
        "story" | "body" => ComponentData::Story(vec![Default::default()]),
        "voice" => ComponentData::Voice(vec![Default::default()]),
        "galleryCharacter" => ComponentData::GalleryCharacter(vec![
            OutputGalleryCharacterItem {
                key: "Character Card".to_string(),
                img: "default_pic.png".to_string(),
                ..Default::default()
            },
            Default::default(),
        ]),
        "artifactList" => ComponentData::ArtifactList(Default::default()),
        "reliquarySetEffect" => ComponentData::ReliquarySetEffect(Default::default()),
        "map" => ComponentData::MapUrl(Default::default()),
        "customize" => ComponentData::Customize(Vec::new()),
        "textualResearch" => ComponentData::TextualResearch(vec![Default::default()]),
        "timeline" => ComponentData::Timeline(Default::default()),
        "videoCollection" => ComponentData::VideoCollection(vec![Default::default()]),
        "tcg" => ComponentData::Tcg(Default::default()),
        "dropMaterial" => ComponentData::DropMaterial(Vec::new()),
        _ => ComponentData::Unknown(Value::Null),
    }
}

fn generate_merged_calendar_example(transformed_data: &TransformedData) -> OutputCalendarFile {
    let mut merged_calendar_items: Option<Vec<OutputCalendarItem>> = None;
    let mut merged_op_items: Option<Vec<OutputCalendarOpItem>> = None;
    let mut first_calendar_metadata: Option<(i64, String)> = None;

    for (lang, calendar_file) in &transformed_data.calendars {
        if first_calendar_metadata.is_none() {
            first_calendar_metadata = Some((calendar_file.version.timestamp(), lang.clone()));
        }
        if merged_calendar_items.is_none() && !calendar_file.calendar.is_empty() {
            let mut example_calendar = calendar_file.calendar.clone();
            if let Some(first_item) = example_calendar.first_mut() {
                if let Some(first_char) = first_item.character_abstracts.first_mut() {
                    if first_char.character_rarity.is_none() {
                        first_char.character_rarity = Some(0);
                    }
                    if first_char.character_vision.is_none() {
                        first_char.character_vision = Some("DefaultVision".to_string());
                    }
                }
                if let Some(first_mat) = first_item.material_abstracts.first_mut() {
                    if first_mat.weapon_rarity.is_none() {
                        first_mat.weapon_rarity = Some(0);
                    }
                }
                if let Some(first_ep) = first_item.ep_abstracts.first_mut() {
                    if first_ep.character_rarity.is_none() && first_ep.weapon_rarity.is_none() {
                        first_ep.character_rarity = Some(0);
                        first_ep.weapon_rarity = Some(0);
                        first_ep.character_vision = Some("DefaultVision".to_string());
                    }
                }
                if first_item.drop_day.is_none() {
                    first_item.drop_day = Some(Value::Array(vec![]));
                }
                if first_item.break_type.is_none() {
                    first_item.break_type = Some(0);
                }
                if first_item.obtain_method.is_none() {
                    first_item.obtain_method = Some("Default Method".to_string());
                }
            }
            merged_calendar_items = Some(example_calendar);
        }
        if merged_op_items.is_none() && !calendar_file.op.is_empty() {
            let mut example_op = calendar_file.op.clone();
            if let Some(first_op) = example_op.first_mut() {
                if let Some(first_ep) = first_op.ep_abstracts.first_mut() {
                    if first_ep.character_rarity.is_none() && first_ep.weapon_rarity.is_none() {
                        first_ep.character_rarity = Some(0);
                        first_ep.weapon_rarity = Some(0);
                        first_ep.character_vision = Some("DefaultVision".to_string());
                    }
                }
                if first_op.is_birth.is_none() {
                    first_op.is_birth = Some(false);
                }
                if first_op.text.is_none() {
                    first_op.text = Some("Default Text".to_string());
                }
                if first_op.title.is_none() {
                    first_op.title = Some("Default Title".to_string());
                }
                if first_op.start_time.is_none() {
                    first_op.start_time = Some("01-01".to_string());
                }
                if first_op.end_time.is_none() {
                    first_op.end_time = Some("12-31".to_string());
                }
            }
            merged_op_items = Some(example_op);
        }
    }

    if merged_calendar_items.is_none() {
        merged_calendar_items = Some(vec![OutputCalendarItem {
            character_abstracts: vec![OutputCalendarAbstract {
                id: 1,
                name: "Char".to_string(),
                icon_url: "char.png".to_string(),
                desc: Some("Desc".to_string()),
                character_rarity: Some(0),
                character_vision: Some("DefaultVision".to_string()),
                ..Default::default()
            }],
            material_abstracts: vec![OutputCalendarAbstract {
                id: 2,
                name: "Mat".to_string(),
                icon_url: "mat.png".to_string(),
                desc: Some("Desc".to_string()),
                weapon_rarity: Some(0),
                ..Default::default()
            }],
            ep_abstracts: vec![OutputCalendarAbstract {
                id: 3,
                name: "Ep".to_string(),
                icon_url: "ep.png".to_string(),
                desc: Some("Desc".to_string()),
                character_rarity: Some(0),
                character_vision: Some("DefaultVision".to_string()),
                weapon_rarity: Some(0),
            }],
            drop_day: Some(Value::Array(vec![Value::from(1)])),
            break_type: Some(0),
            obtain_method: Some("Default Method".to_string()),
        }]);
    }
    if merged_op_items.is_none() {
        merged_op_items = Some(vec![OutputCalendarOpItem {
            ep_abstracts: vec![OutputCalendarAbstract {
                id: 4,
                name: "OpEp".to_string(),
                icon_url: "op_ep.png".to_string(),
                desc: Some("Desc".to_string()),
                character_rarity: Some(0),
                character_vision: Some("DefaultVision".to_string()),
                weapon_rarity: Some(0),
            }],
            is_birth: Some(false),
            text: Some("Default Text".to_string()),
            title: Some("Default Title".to_string()),
            start_time: Some("01-01".to_string()),
            end_time: Some("12-31".to_string()),
        }]);
    }

    let (version_ts, lang) = first_calendar_metadata
        .unwrap_or_else(|| (Utc::now().timestamp(), "default_lang".to_string()));

    OutputCalendarFile {
        version: DateTime::from_timestamp(version_ts, 0).unwrap_or_else(Utc::now),
        language: lang,
        calendar: merged_calendar_items.unwrap_or_default(),
        op: merged_op_items.unwrap_or_default(),
    }
}
