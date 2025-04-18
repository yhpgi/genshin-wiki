use crate::api::client::ApiClient;
use crate::api::fetchers;
use crate::config;
use crate::core::data_store::InMemoryDataStore;
use crate::core::merged_schema;
use crate::core::stats::{self, CategoryStats};
use crate::error::{AppError, AppResult};
use crate::io;
use crate::logging::{log, LogLevel};
use crate::model::common::{EntryId, MenuId};
use crate::transform::{self, bulk::BulkStore};
use chrono::Utc;
use futures::stream::{self, StreamExt};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::fs;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

pub async fn run(target_langs: Vec<String>, out_dir: PathBuf) -> AppResult<i32> {
    let overall_start_time = Instant::now();
    let start_ts_str = Utc::now().format("%Y-%m-%d %H:%M:%S %Z").to_string();

    log(
        LogLevel::Step,
        &format!(
            "Starting Wiki Update for {} language(s) at {}",
            target_langs.len(),
            start_ts_str
        ),
    );
    log(
        LogLevel::Info,
        &format!("Output Directory: {}", out_dir.display()),
    );

    let client = Arc::new(ApiClient::new()?);

    let list_sem = Arc::new(Semaphore::new(config::MAX_LIST_CONCUR));
    let detail_sem = Arc::new(Semaphore::new(config::MAX_DETAIL_CONCUR));
    let bulk_sem = Arc::new(Semaphore::new(config::MAX_BULK_CONCUR));
    let cal_sem = Arc::new(Semaphore::new(config::MAX_CALENDAR_CONCUR));

    let mut run_stats = stats::initialize_stats();
    let mut data_store = InMemoryDataStore::default();

    let nav_start_time = Instant::now();
    log(LogLevel::Step, "--- Phase 1: Navigation Fetch ---");
    run_stats
        .get_mut("Navigation")
        .unwrap()
        .set_total(target_langs.len());
    let mut nav_tasks = JoinSet::new();
    for lang in target_langs.iter() {
        let client_clone = client.clone();
        let lang_clone = lang.clone();
        nav_tasks.spawn(async move {
            let result = fetchers::fetch_nav(&client_clone, &lang_clone).await;
            (lang_clone, result)
        });
    }
    while let Some(result) = nav_tasks.join_next().await {
        let stats_nav = run_stats.get_mut("Navigation").unwrap();
        match result {
            Ok((lang, Ok(nav_entries))) => {
                if nav_entries.is_empty() {
                    log(
                        LogLevel::Warning,
                        &format!("Navigation fetch for [{}] returned empty.", lang),
                    );
                    stats_nav.add_skip();
                } else {
                    stats_nav.add_ok();
                    data_store.raw.navigation.insert(lang, nav_entries);
                }
            }
            Ok((lang, Err(e))) => {
                log(
                    LogLevel::Warning,
                    &format!(
                        "Navigation fetch failed for [{}]: {:?}, skipping language.",
                        lang, e
                    ),
                );
                stats_nav.add_fail();
            }
            Err(e) => {
                log(LogLevel::Error, &format!("Nav fetch task panicked: {}", e));
                stats_nav.add_fail();
            }
        }
    }
    log_phase_completion(
        "Navigation Fetch",
        &run_stats["Navigation"],
        nav_start_time.elapsed(),
    );
    if data_store.raw.navigation.is_empty() && !target_langs.is_empty() {
        log(
            LogLevel::Error,
            "Critical: No navigation data fetched for any language. Cannot proceed.",
        );
        return Ok(1);
    }

    let list_start_time = Instant::now();
    log(LogLevel::Step, "--- Phase 2: List Fetch ---");
    let mut list_fetch_input: Vec<(String, MenuId, String)> = Vec::new();
    for (lang, nav_entries) in &data_store.raw.navigation {
        let lang_list_map = data_store.raw.lists.entry(lang.clone()).or_default();
        for entry in nav_entries {
            if let Some(menu) = transform::common::transform_nav_item(entry) {
                list_fetch_input.push((lang.clone(), menu.menu_id, menu.name));
                lang_list_map.insert(menu.menu_id, Vec::new());
            }
        }
    }
    let total_list_tasks = list_fetch_input.len();
    run_stats
        .get_mut("List Fetch")
        .unwrap()
        .set_total(total_list_tasks);
    let list_processed_count = Arc::new(AtomicUsize::new(0));
    let list_log_interval = std::cmp::max(10, (total_list_tasks / 10).max(1));

    if !list_fetch_input.is_empty() {
        log(
            LogLevel::Info,
            &format!("Fetching {} Lists...", total_list_tasks),
        );
        let list_stream = stream::iter(list_fetch_input)
            .map(|(lang, menu_id, menu_name)| {
                let client_c = client.clone();
                let list_sem_c = list_sem.clone();
                let menu_name_c = menu_name;
                async move {
                    let result = fetchers::fetch_menu_list_items(
                        &client_c,
                        list_sem_c,
                        &lang,
                        menu_id,
                        &menu_name_c,
                    )
                    .await;
                    (lang, menu_id, result)
                }
            })
            .buffer_unordered(config::MAX_LIST_CONCUR * 2);

        list_stream.for_each(|(lang, menu_id, result)| {
             let stats_list = run_stats.get_mut("List Fetch").unwrap();
             let current_processed = list_processed_count.fetch_add(1, Ordering::Relaxed) + 1;
             match result {
                 Ok(items) => {
                     if items.is_empty() {
                        stats_list.add_skip();
                    } else {
                        stats_list.add_ok();
                        if let Some(lm) = data_store.raw.lists.get_mut(&lang) {
                            lm.insert(menu_id, items);
                         } else {
                             log(LogLevel::Warning, &format!("List Fetch: Could not find language map entry for [{}] after fetch", lang));
                         }
                     }
                }
                Err(e) => {
                    log(LogLevel::Warning, &format!("List Fetch Error for Menu {} [{}]: {:?} - Marking as Skipped", menu_id, lang, e));
                     stats_list.add_skip();
                }
             }
             if current_processed % list_log_interval == 0 || current_processed == total_list_tasks {
                log_progress("List Fetch", stats_list, current_processed);
             }
             futures::future::ready(())
         }).await;
    } else {
        log(
            LogLevel::Warning,
            "No lists to fetch (no valid navigation items found).",
        );
    }
    log_phase_completion(
        "List Fetch",
        &run_stats["List Fetch"],
        list_start_time.elapsed(),
    );

    let detail_start_time = Instant::now();
    log(LogLevel::Step, "--- Phase 3: Detail Fetch ---");
    let mut detail_fetch_input: Vec<(String, EntryId)> = Vec::new();
    for (lang, lists_map) in &data_store.raw.lists {
        for items in lists_map.values() {
            for item in items {
                if item.entry_page_id > 0 {
                    detail_fetch_input.push((lang.clone(), item.entry_page_id));
                }
            }
        }
    }
    detail_fetch_input.sort_unstable_by_key(|k| (k.0.clone(), k.1));
    detail_fetch_input.dedup();

    let total_detail_tasks = detail_fetch_input.len();
    run_stats
        .get_mut("Detail Fetch")
        .unwrap()
        .set_total(total_detail_tasks);
    let detail_processed_count = Arc::new(AtomicUsize::new(0));
    let detail_log_interval = std::cmp::max(50, (total_detail_tasks / 20).max(1));

    if !detail_fetch_input.is_empty() {
        log(
            LogLevel::Info,
            &format!("Fetching {} unique Details...", total_detail_tasks),
        );
        let detail_stream = stream::iter(detail_fetch_input)
            .map(|(lang, entry_id)| {
                let client_c = client.clone();
                let detail_sem_c = detail_sem.clone();
                async move {
                    let result =
                        fetchers::fetch_entry_detail(&client_c, detail_sem_c, &lang, entry_id)
                            .await;
                    (lang, result)
                }
            })
            .buffer_unordered(config::MAX_DETAIL_CONCUR * 2);

        detail_stream
            .for_each(|(lang, result)| {
                let stats_detail = run_stats.get_mut("Detail Fetch").unwrap();
                let current_processed = detail_processed_count.fetch_add(1, Ordering::Relaxed) + 1;
                match result {
                    Ok(Some(detail_page)) => {
                        stats_detail.add_ok();
                        data_store
                            .raw
                            .details
                            .entry(lang)
                            .or_default()
                            .push(detail_page);
                    }
                    Ok(None) => {
                        stats_detail.add_skip();
                    }
                    Err(e) => {
                        log(
                            LogLevel::Warning,
                            &format!(
                                "Detail Fetch Error for [{}]: {:?} - Marking as Skipped",
                                lang, e
                            ),
                        );
                        stats_detail.add_skip();
                    }
                }
                if current_processed % detail_log_interval == 0
                    || current_processed == total_detail_tasks
                {
                    log_progress("Detail Fetch", stats_detail, current_processed);
                }
                futures::future::ready(())
            })
            .await;
    } else {
        log(
            LogLevel::Warning,
            "No details to fetch (no items found in lists).",
        );
    }
    log_phase_completion(
        "Detail Fetch",
        &run_stats["Detail Fetch"],
        detail_start_time.elapsed(),
    );

    let calendar_start_time = Instant::now();
    log(LogLevel::Step, "--- Phase 4: Calendar Fetch ---");
    run_stats
        .get_mut("Calendar Fetch")
        .unwrap()
        .set_total(target_langs.len());
    let mut cal_tasks = JoinSet::new();
    for lang in target_langs.iter() {
        let client_clone = client.clone();
        let cal_sem_clone = cal_sem.clone();
        let lang_clone = lang.clone();
        cal_tasks.spawn(async move {
            let result = fetchers::fetch_calendar(&client_clone, cal_sem_clone, &lang_clone).await;
            (lang_clone, result)
        });
    }
    while let Some(result) = cal_tasks.join_next().await {
        let stats_cal = run_stats.get_mut("Calendar Fetch").unwrap();
        match result {
            Ok((lang, Ok(calendar_data))) => {
                if calendar_data.calendar.is_empty() && calendar_data.op.is_empty() {
                    stats_cal.add_skip();
                } else {
                    stats_cal.add_ok();
                    data_store.raw.calendars.insert(lang, calendar_data);
                }
            }
            Ok((lang, Err(e))) => {
                log(
                    LogLevel::Warning,
                    &format!("Calendar fetch failed for [{}]: {:?}", lang, e),
                );
                stats_cal.add_fail();
            }
            Err(e) => {
                log(
                    LogLevel::Error,
                    &format!("Calendar fetch task panicked: {}", e),
                );
                stats_cal.add_fail();
            }
        }
    }
    log_phase_completion(
        "Calendar Fetch",
        &run_stats["Calendar Fetch"],
        calendar_start_time.elapsed(),
    );

    let bulk_start_time = Instant::now();
    log(LogLevel::Step, "--- Phase 5: Bulk Data Fetch & Process ---");
    log(LogLevel::Info, "Collecting all unique Entry IDs...");
    data_store.all_ids = transform::collect_all_ids(&data_store.raw);
    let total_unique_ids: usize = data_store.all_ids.values().map(HashSet::len).sum();
    log(
        LogLevel::Success,
        &format!(
            "Collected {} unique ID(s) across {} language(s).",
            total_unique_ids,
            data_store.all_ids.len()
        ),
    );

    let mut lang_bulk_processing_tasks = JoinSet::new();
    let primary_bulk_processed_ids = Arc::new(AtomicUsize::new(0));

    let total_primary_fetch_units: usize = data_store
        .all_ids
        .values()
        .map(|ids| (ids.len() + config::BULK_BATCH_SIZE - 1) / config::BULK_BATCH_SIZE)
        .sum();
    let total_fallback_fetch_units: usize = data_store
        .all_ids
        .iter()
        .map(|(_lang, ids)| {
            let fallback_langs_count = config::SUPPORTED_LANGS.len().saturating_sub(1);
            ids.len() * fallback_langs_count / config::BULK_BATCH_SIZE.max(1)
        })
        .sum();

    run_stats
        .get_mut("Bulk Primary")
        .unwrap()
        .set_total(total_primary_fetch_units);
    run_stats
        .get_mut("Bulk Fallback")
        .unwrap()
        .set_total(total_fallback_fetch_units);
    let bulk_log_interval = std::cmp::max(10, (total_primary_fetch_units / 10).max(1));

    if total_unique_ids > 0 {
        log(
            LogLevel::Info,
            &format!(
                "Processing Bulk data for {} primary language(s)...",
                data_store.all_ids.len()
            ),
        );

        for lang in target_langs.iter() {
            let ids_for_lang_arc =
                Arc::new(data_store.all_ids.get(lang).cloned().unwrap_or_default());

            if ids_for_lang_arc.is_empty() {
                log(
                    LogLevel::Info,
                    &format!(
                        "Bulk: Skipping language [{}] as no IDs were collected.",
                        lang
                    ),
                );
                data_store
                    .all_bulk_stores
                    .insert(lang.clone(), BulkStore::default());
                continue;
            }

            let lang_clone = lang.clone();
            let client_clone = client.clone();
            let bulk_sem_clone = bulk_sem.clone();
            let primary_counter_clone = primary_bulk_processed_ids.clone();

            lang_bulk_processing_tasks.spawn(async move {

                let primary_fetch_result = fetchers::fetch_bulk_data(&client_clone, bulk_sem_clone.clone(), &ids_for_lang_arc, &lang_clone, "Bulk Primary").await;
                 let (primary_bulk_map, prim_ok_count, prim_fail_skip_count) = match primary_fetch_result {
                    Ok(map) => (map.clone(), (map.len() + config::BULK_BATCH_SIZE -1)/ config::BULK_BATCH_SIZE.max(1) , 0 ),
                    Err(e) => {
                         log(LogLevel::Warning, &format!("Bulk Primary fetch failed entirely for [{}]: {:?}", lang_clone, e));

                         (HashMap::new(), 0, (ids_for_lang_arc.len() + config::BULK_BATCH_SIZE -1) / config::BULK_BATCH_SIZE.max(1) )
                    }
                 };
                primary_counter_clone.fetch_add( (ids_for_lang_arc.len() + config::BULK_BATCH_SIZE - 1) / config::BULK_BATCH_SIZE.max(1), Ordering::Relaxed);


                 let mut ids_needing_fallback = HashSet::new();
                 for id in ids_for_lang_arc.iter() {
                     let needs_fallback = primary_bulk_map.get(id)
                        .and_then(|p| p.icon_url.as_deref())
                         .map_or(true, |icon| icon.is_empty() || icon.contains("invalid-file"));
                     if needs_fallback { ids_needing_fallback.insert(*id); }
                 }

                 let mut lang_fallback_map: HashMap<String, HashMap<EntryId, crate::api::model::ApiBulkPage>> = HashMap::new();
                 let mut fallback_ok_batches = 0;
                 let mut fallback_fail_batches = 0;

                 if !ids_needing_fallback.is_empty() {
                    let mut fallback_tasks = JoinSet::new();
                     let fallback_langs: Vec<String> = config::SUPPORTED_LANGS.iter()
                         .filter(|&fl| *fl != lang_clone)
                         .cloned()
                         .collect();

                     let ids_needing_fallback_arc = Arc::new(ids_needing_fallback);

                     for fallback_lang in fallback_langs {
                         let client_c = client_clone.clone();
                        let bulk_sem_c = bulk_sem_clone.clone();
                        let lang_c = fallback_lang;
                        let ids_c = ids_needing_fallback_arc.clone();
                        let ctx = format!("Bulk Fallback ({}) for [{}]", lang_c, lang_clone);

                         fallback_tasks.spawn(async move {
                             let result = fetchers::fetch_bulk_data(&client_c, bulk_sem_c, &ids_c, &lang_c, &ctx).await;
                             let batches = (ids_c.len() + config::BULK_BATCH_SIZE -1) / config::BULK_BATCH_SIZE.max(1);
                             (lang_c, result.map(|map| (map, batches)))
                         });
                    }

                    while let Some(fall_result) = fallback_tasks.join_next().await {
                         match fall_result {
                            Ok((lang_key, Ok((map, batches)))) => {
                                 if !map.is_empty() {
                                     lang_fallback_map.insert(lang_key, map);
                                 }
                                 fallback_ok_batches += batches;
                             }
                            Ok((lang_key, Err(_))) => {
                                log(LogLevel::Warning, &format!("Bulk Fallback fetch failed entirely for [{}] (fallback for [{}])", lang_key, lang_clone));
                                fallback_fail_batches += (ids_needing_fallback_arc.len() + config::BULK_BATCH_SIZE - 1) / config::BULK_BATCH_SIZE.max(1);
                            }
                             Err(e) => {
                                log(LogLevel::Error, &format!("Bulk Fallback task panicked: {}", e));
                                fallback_fail_batches += (ids_needing_fallback_arc.len() + config::BULK_BATCH_SIZE - 1) / config::BULK_BATCH_SIZE.max(1);
                            }
                        }
                    }
                 }

                 match transform::bulk::process_bulk_data(primary_bulk_map, lang_fallback_map, &ids_for_lang_arc) {
                    Ok(store) => Ok((lang_clone, store, prim_ok_count, prim_fail_skip_count, fallback_ok_batches, fallback_fail_batches)),
                     Err(e) => {
                         log(LogLevel::Error, &format!("Bulk processing failed for [{}]: {:?}.", lang_clone, e));
                         Err(AppError::TransformError(format!("Bulk processing failed for {}", lang_clone)))
                     }
                 }
            });
        }

        while let Some(join_result) = lang_bulk_processing_tasks.join_next().await {
            match join_result {
                Ok(Ok((lang, store, prim_ok, prim_fail_skip, fall_ok, fall_fail_skip))) => {
                    data_store.all_bulk_stores.insert(lang.clone(), store);
                    run_stats.get_mut("Bulk Primary").unwrap().ok += prim_ok;
                    run_stats.get_mut("Bulk Primary").unwrap().fail += prim_fail_skip;
                    run_stats.get_mut("Bulk Fallback").unwrap().ok += fall_ok;
                    run_stats.get_mut("Bulk Fallback").unwrap().fail += fall_fail_skip;

                    let current_processed = primary_bulk_processed_ids.load(Ordering::Relaxed);
                    if current_processed % bulk_log_interval == 0
                        || current_processed >= total_primary_fetch_units
                    {
                        log_progress(
                            "Bulk Primary",
                            &run_stats["Bulk Primary"],
                            current_processed,
                        );
                    }
                }
                Ok(Err(e)) => {
                    log(
                        LogLevel::Error,
                        &format!("Bulk processing task failed: {:?}", e),
                    );
                    run_stats.get_mut("Bulk Primary").unwrap().add_fail();
                }
                Err(e) => {
                    log(
                        LogLevel::Error,
                        &format!("Bulk processing task panicked: {}", e),
                    );
                    run_stats.get_mut("Bulk Primary").unwrap().add_fail();
                }
            }
        }
    } else {
        log(
            LogLevel::Warning,
            "No unique IDs found; skipping bulk data fetch entirely.",
        );
    }
    log_phase_completion(
        "Bulk Data Fetch (Primary Batches)",
        &run_stats["Bulk Primary"],
        bulk_start_time.elapsed(),
    );
    log_phase_completion(
        "Bulk Data Fetch (Fallback Batches)",
        &run_stats["Bulk Fallback"],
        bulk_start_time.elapsed(),
    );

    let transform_start_time = Instant::now();
    log(LogLevel::Step, "--- Phase 6: Transforming Data ---");
    let transformed_data = transform::transform_all_data(
        Arc::new(data_store.raw),
        data_store.all_bulk_stores,
        &target_langs,
    )
    .await?;
    let transformed_data_arc = Arc::new(transformed_data);
    log_phase_completion(
        "Transforming Data",
        &CategoryStats::default(),
        transform_start_time.elapsed(),
    );

    let save_start_time = Instant::now();
    log(LogLevel::Step, "--- Phase 7: Saving Transformed Data ---");
    io::ensure_output_directories(&out_dir).await?;
    let mut save_tasks = JoinSet::new();
    let mut total_files_to_save = 0usize;

    let nav_base_dir = out_dir.join("navigation");
    if transformed_data_arc
        .navigation
        .values()
        .any(|v| !v.is_empty())
    {
        fs::create_dir_all(&nav_base_dir).await?;
        for (lang, nav_items) in transformed_data_arc.navigation.iter() {
            if nav_items.is_empty() {
                continue;
            }
            let path = nav_base_dir.join(format!("{}.json", lang));
            let ctx = format!("Nav [{}]", lang);
            total_files_to_save += 1;
            let nav_items_clone = nav_items.clone();
            save_tasks.spawn(io::save_json(path, nav_items_clone, ctx));
        }
    }

    let list_base_dir = out_dir.join("list");
    if transformed_data_arc.lists.values().any(|v| !v.is_empty()) {
        fs::create_dir_all(&list_base_dir).await?;
        for (lang, list_files) in transformed_data_arc.lists.iter() {
            if list_files.is_empty() {
                continue;
            }
            let lang_list_dir = list_base_dir.join(lang);
            fs::create_dir_all(&lang_list_dir).await?;
            for list_file in list_files {
                let file_name = format!("{}.json", list_file.menu_id);
                let path = lang_list_dir.join(file_name);
                let ctx = format!("List M:{} [{}]", list_file.menu_id, lang);
                total_files_to_save += 1;
                let list_file_clone = list_file.clone();
                save_tasks.spawn(io::save_json(path, list_file_clone, ctx));
            }
        }
    }

    let detail_base_dir = out_dir.join("detail");
    if transformed_data_arc.details.values().any(|v| !v.is_empty()) {
        fs::create_dir_all(&detail_base_dir).await?;
        for (lang, detail_pages) in transformed_data_arc.details.iter() {
            if detail_pages.is_empty() {
                continue;
            }
            let lang_detail_dir = detail_base_dir.join(lang);
            fs::create_dir_all(&lang_detail_dir).await?;
            for detail_page in detail_pages {
                let file_name = format!("{}.json", detail_page.id);
                let path = lang_detail_dir.join(file_name);
                let ctx = format!("Detail E:{} [{}]", detail_page.id, lang);
                total_files_to_save += 1;
                let detail_page_clone = detail_page.clone();
                save_tasks.spawn(io::save_json(path, detail_page_clone, ctx));
            }
        }
    }

    let calendar_base_dir = out_dir.join("calendar");
    if !transformed_data_arc.calendars.is_empty() {
        fs::create_dir_all(&calendar_base_dir).await?;
        for (lang, calendar_file) in transformed_data_arc.calendars.iter() {
            let path = calendar_base_dir.join(format!("{}.json", lang));
            let ctx = format!("Calendar [{}]", lang);
            total_files_to_save += 1;
            let calendar_file_clone = calendar_file.clone();
            save_tasks.spawn(io::save_json(path, calendar_file_clone, ctx));
        }
    }

    run_stats
        .get_mut("Save Files")
        .unwrap()
        .set_total(total_files_to_save);
    if total_files_to_save > 0 {
        log(
            LogLevel::Info,
            &format!("Saving {} files...", total_files_to_save),
        );
        while let Some(result) = save_tasks.join_next().await {
            let stats_save = run_stats.get_mut("Save Files").unwrap();
            match result {
                Ok(Ok(true)) => {
                    stats_save.add_ok();
                }

                Ok(Err(e)) => {
                    stats_save.add_fail();
                    log(
                        LogLevel::Error,
                        &format!("Save task failed internally: {:?}", e),
                    );
                }
                Err(e) => {
                    stats_save.add_fail();
                    log(LogLevel::Error, &format!("Save task panicked: {}", e));
                }

                Ok(Ok(false)) => {
                    stats_save.add_fail();
                    log(
                        LogLevel::Error,
                        "Save task reported failure (returned false).",
                    );
                }
            }
        }
    } else {
        log(
            LogLevel::Warning,
            "No transformed data files generated to save.",
        );
    }
    log_phase_completion(
        "Save Files",
        &run_stats["Save Files"],
        save_start_time.elapsed(),
    );

    let schema_start_time = Instant::now();
    log(
        LogLevel::Step,
        "--- Phase 8: Creating Merged Schema Files ---",
    );
    match merged_schema::create_merged_schema_files(transformed_data_arc, &out_dir).await {
        Ok(_) => log_phase_completion(
            "Schema Generation",
            &CategoryStats {
                ok: 1,
                ..Default::default()
            },
            schema_start_time.elapsed(),
        ),
        Err(e) => {
            log(
                LogLevel::Error,
                &format!("Failed to generate merged schema files: {:?}", e),
            );

            run_stats
                .entry("Schema Generation".to_string())
                .or_default()
                .add_fail();
        }
    }

    let overall_duration = overall_start_time.elapsed();
    stats::print_summary(&run_stats, &target_langs, overall_duration);
    let exit_code = stats::determine_exit_code(&run_stats);

    Ok(exit_code)
}

fn log_progress(phase: &str, stats: &CategoryStats, current_processed: usize) {
    if stats.total_tasks == 0 {
        return;
    }
    let percentage = (current_processed as f32 / stats.total_tasks.max(1) as f32) * 100.0;
    log(
        LogLevel::Info,
        &format!(
            "{} progress: {}/{} ({:.1}%) [OK: {}, Skip: {}, Fail: {}]",
            phase,
            current_processed,
            stats.total_tasks,
            percentage,
            stats.ok,
            stats.skip_or_empty,
            stats.fail
        ),
    );
}

fn log_phase_completion(phase: &str, stats: &CategoryStats, elapsed: Duration) {
    let level = if stats.fail > 0 {
        LogLevel::Warning
    } else {
        LogLevel::Success
    };
    let phase_name = format!("{} Phase", phase);

    let should_log = stats.total_tasks > 0
        || matches!(
            phase,
            "Navigation Fetch"
                | "Calendar Fetch"
                | "Transforming Data"
                | "Save Files"
                | "Schema Generation"
        )
        || phase.contains("Bulk Data");

    if should_log {
        log(
            level,
            &format!(
                "--- {} complete ({} OK, {} Skip/Empty, {} Fail / {} Total) | Elapsed: {:?} ---",
                phase_name, stats.ok, stats.skip_or_empty, stats.fail, stats.total_tasks, elapsed
            ),
        );
    }
}
