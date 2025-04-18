use crate::api::model::ApiBulkPage;
use crate::error::AppResult;
use crate::model::common::EntryId;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub struct BulkInfo {
    pub name: Option<String>,
    pub desc: Option<String>,
    pub best_icon_url: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct BulkStore(Arc<HashMap<EntryId, BulkInfo>>);

impl BulkStore {
    #[inline]
    pub fn get(&self, id: &EntryId) -> Option<&BulkInfo> {
        self.0.get(id)
    }
    #[inline]
    pub fn contains_key(&self, id: &EntryId) -> bool {
        self.0.contains_key(id)
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[inline]
    pub fn get_name(&self, id: EntryId) -> Option<&str> {
        self.0.get(&id).and_then(|info| info.name.as_deref())
    }

    #[inline]
    pub fn get_icon(&self, id: EntryId) -> Option<&str> {
        self.0
            .get(&id)
            .and_then(|info| info.best_icon_url.as_deref())
    }

    #[inline]
    pub fn get_desc(&self, id: EntryId) -> Option<&str> {
        self.0.get(&id).and_then(|info| info.desc.as_deref())
    }
}

pub fn process_bulk_data(
    primary_bulk: HashMap<EntryId, ApiBulkPage>,
    fallback_map: HashMap<String, HashMap<EntryId, ApiBulkPage>>,
    all_ids_for_primary_lang: &HashSet<EntryId>,
) -> AppResult<BulkStore> {
    let mut store_map = HashMap::with_capacity(all_ids_for_primary_lang.len());

    for &id in all_ids_for_primary_lang {
        let mut info = BulkInfo::default();
        let mut primary_icon_valid = false;

        if let Some(primary_page) = primary_bulk.get(&id) {
            info.name = primary_page.name.clone();
            info.desc = primary_page.desc.clone();
            if let Some(icon) = primary_page.icon_url.as_deref() {
                if !icon.is_empty() && !icon.contains("invalid-file") {
                    info.best_icon_url = Some(icon.to_string());
                    primary_icon_valid = true;
                }
            }
        }

        if !primary_icon_valid {
            for fallback_bulk in fallback_map.values() {
                if let Some(fallback_page) = fallback_bulk.get(&id) {
                    if let Some(fallback_icon) = fallback_page.icon_url.as_deref() {
                        if !fallback_icon.is_empty() && !fallback_icon.contains("invalid-file") {
                            info.best_icon_url = Some(fallback_icon.to_string());
                            break;
                        }
                    }
                }
            }
        }

        if info.name.is_some() || info.desc.is_some() || info.best_icon_url.is_some() {
            store_map.insert(id, info);
        }
    }
    Ok(BulkStore(Arc::new(store_map)))
}

#[inline]
pub fn resolve_icon(id: EntryId, bulk_store: &BulkStore) -> Option<String> {
    bulk_store.get_icon(id).map(String::from)
}

#[inline]
pub fn resolve_name(id: EntryId, bulk_store: &BulkStore) -> Option<String> {
    bulk_store.get_name(id).map(String::from)
}

#[inline]
pub fn resolve_desc(id: EntryId, bulk_store: &BulkStore) -> Option<String> {
    bulk_store.get_desc(id).map(String::from)
}
