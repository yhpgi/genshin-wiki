use crate::api;
use crate::model::{
    common::MenuId,
    output::{OutputListFile, OutputListItem},
};
use crate::transform::{bulk, common};
use chrono::Utc;

pub fn transform_list_file(
    raw_items: Vec<api::model::ApiListItem>,
    bulk_store: &bulk::BulkStore,
    lang: &str,
    menu_id: MenuId,
    menu_name: String,
) -> Option<OutputListFile> {
    let initial_count = raw_items.len();
    if initial_count == 0 {
        return None;
    }

    let mut output_items = Vec::with_capacity(initial_count);

    for item in raw_items {
        let item_id = item.entry_page_id;

        let name = bulk_store.get_name(item_id).unwrap_or(&item.name);
        let icon_url = bulk_store.get_icon(item_id).unwrap_or(&item.icon_url);
        let desc = bulk_store.get_desc(item_id).map(String::from).or(item.desc);

        let filter_values = common::process_filters_value(&item.filter_values);

        if !name.is_empty() || !icon_url.is_empty() {
            output_items.push(OutputListItem {
                id: item_id,
                name: name.to_string(),
                icon_url: icon_url.to_string(),
                desc,
                filter_values,
            });
        }
    }

    let final_count = output_items.len();
    if final_count == 0 {
        None
    } else {
        output_items.sort_unstable_by_key(|item| item.id);
        Some(OutputListFile {
            version: Utc::now(),
            language: lang.to_string(),
            menu_id,
            menu_name,
            total_items: final_count,
            list: output_items,
        })
    }
}
