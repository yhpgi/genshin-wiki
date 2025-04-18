use crate::api;
use crate::model;
use crate::model::common::{EntryId, MenuId};
use crate::transform::bulk::BulkStore;
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default)]
pub struct RawData {
    pub navigation: HashMap<String, Vec<api::model::ApiNavEntry>>,
    pub lists: HashMap<String, HashMap<MenuId, Vec<api::model::ApiListItem>>>,
    pub details: HashMap<String, Vec<api::model::ApiDetailPage>>,
    pub calendars: HashMap<String, api::model::ApiCalendarResponse>,
}

#[derive(Debug, Default, JsonSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransformedData {
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub navigation: HashMap<String, Vec<model::output::OutputNavMenuItem>>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub lists: HashMap<String, Vec<model::output::OutputListFile>>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub details: HashMap<String, Vec<model::output::OutputDetailPage>>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub calendars: HashMap<String, model::output::OutputCalendarFile>,
}

#[derive(Default)]
pub struct InMemoryDataStore {
    pub raw: RawData,
    pub transformed: TransformedData,
    pub all_ids: HashMap<String, HashSet<EntryId>>,
    pub all_bulk_stores: HashMap<String, BulkStore>,
}
