use super::client::ApiClient;
use super::model::{
    ApiBulkPage, ApiBulkResponse, ApiCalendarResponse, ApiDetailPage, ApiDetailResponse,
    ApiListItem, ApiListResponse, ApiNavEntry, ApiNavResponse,
};
use crate::config;
use crate::error::{AppError, AppResult};
use crate::logging::{log, LogLevel};
use crate::model::common::{EntryId, MenuId};
use crate::utils;
use reqwest::Method;

use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

pub async fn fetch_nav(client: &ApiClient, lang: &str) -> AppResult<Vec<ApiNavEntry>> {
    let endpoint_name = "nav";

    match client
        .fetch::<ApiNavResponse>(Method::GET, endpoint_name, lang, None, None)
        .await
    {
        Ok(data) => Ok(data.nav),
        Err(e) => {
            log(
                LogLevel::Warning,
                &format!("Nav Fetch FAIL [{}]: {:?}", lang, e),
            );
            Err(e)
        }
    }
}

pub async fn fetch_menu_list_items(
    client: &ApiClient,
    list_sem: Arc<Semaphore>,
    lang: &str,
    menu_id: MenuId,
    menu_name: &str,
) -> AppResult<Vec<ApiListItem>> {
    let mut all_items: Vec<ApiListItem> = Vec::new();
    let mut total_from_api: Option<i64> = None;
    let mut current_page = 1;
    let ctx = format!("List Menu:{} ('{}') [{}]", menu_id, menu_name, lang);
    let endpoint_name = "list";

    let effective_menu_id = if menu_id == 0 { Some(9) } else { Some(menu_id) };

    loop {
        let payload = json!({
            "menu_id": effective_menu_id,
            "page_num": current_page,
            "page_size": config::PAGE_SIZE,
            "use_es": true,

            "filters": if effective_menu_id == Some(9) { Some(json!([])) } else { None }
        });

        let permit = utils::acquire_semaphore(&list_sem, "List Fetch Page").await?;
        let fetch_result = client
            .fetch::<ApiListResponse>(Method::POST, endpoint_name, lang, None, Some(&payload))
            .await;
        drop(permit);

        match fetch_result {
            Ok(resp_data) => {
                if total_from_api.is_none() {
                    total_from_api = resp_data.total;
                }
                let current_page_items = resp_data.list;
                let count = current_page_items.len();

                if count == 0 {
                    break;
                }

                all_items.extend(current_page_items);

                if let Some(expected_total) = total_from_api {
                    if all_items.len() as i64 >= expected_total {
                        if all_items.len() as i64 > expected_total {
                            log(
                                LogLevel::Warning,
                                &format!(
                                    "{} - Fetched more items than expected ({} > {}). Truncating.",
                                    ctx,
                                    all_items.len(),
                                    expected_total
                                ),
                            );
                            all_items.truncate(expected_total.try_into().unwrap_or(0));
                        }
                        break;
                    }
                }
                current_page += 1;
            }

            Err(AppError::ApiError {
                retcode: 100010, ..
            }) => {
                if current_page == 1 {
                    log(
                        LogLevel::Warning,
                        &format!(
                            "{} - First page request indicated no data (100010). Returning empty.",
                            ctx
                        ),
                    );
                } else {
                    log(
                        LogLevel::Info,
                        &format!(
                            "{} - Pagination ended with 'Not Found' (100010) after page {}.",
                            ctx,
                            current_page - 1
                        ),
                    );
                }
                break;
            }

            Err(e) => {
                log(
                    LogLevel::Warning,
                    &format!("{} - Fetch FAIL Page {}: {:?}.", ctx, current_page, e),
                );

                return Err(e);
            }
        }
    }

    Ok(all_items)
}

pub async fn fetch_entry_detail(
    client: &ApiClient,
    detail_sem: Arc<Semaphore>,
    lang: &str,
    entry_id: EntryId,
) -> AppResult<Option<ApiDetailPage>> {
    let endpoint_name = "detail";
    let params = HashMap::from([("entry_page_id".to_string(), entry_id.to_string())]);
    let permit = utils::acquire_semaphore(&detail_sem, "Detail Fetch Item").await?;
    let fetch_result = client
        .fetch::<ApiDetailResponse>(Method::GET, endpoint_name, lang, Some(&params), None)
        .await;
    drop(permit);

    match fetch_result {
        Ok(data) => Ok(Some(data.page)),

        Err(AppError::ApiError {
            retcode: 100010, ..
        }) => {
            log(
                LogLevel::Info,
                &format!(
                    "Detail Fetch [{}/{}] - Item not found (100010).",
                    lang, entry_id
                ),
            );
            Ok(None)
        }
        Err(e) => {
            log(
                LogLevel::Warning,
                &format!("Detail Fetch FAIL [{}/{}]: {:?}.", lang, entry_id, e),
            );
            Err(e)
        }
    }
}

pub async fn fetch_calendar(
    client: &ApiClient,
    cal_sem: Arc<Semaphore>,
    lang: &str,
) -> AppResult<ApiCalendarResponse> {
    let endpoint_name = "calendar";
    let log_ctx = format!("Calendar [{}]", lang);
    let permit = utils::acquire_semaphore(&cal_sem, "Calendar Fetch").await?;

    let result = client
        .fetch::<ApiCalendarResponse>(Method::GET, endpoint_name, lang, None, None)
        .await;
    drop(permit);

    match result {
        Ok(data) => {
            if data.calendar.is_empty() && data.op.is_empty() {
                log(
                    LogLevel::Info,
                    &format!("{} - Fetch OK but returned empty data.", log_ctx),
                );
            }
            Ok(data)
        }
        Err(e) => {
            log(
                LogLevel::Warning,
                &format!("{} - Fetch FAIL: {:?}", log_ctx, e),
            );
            Err(e)
        }
    }
}

pub async fn fetch_bulk_data(
    client: &ApiClient,
    bulk_sem: Arc<Semaphore>,
    ids: &HashSet<EntryId>,
    lang: &str,
    log_ctx_prefix: &str,
) -> AppResult<HashMap<EntryId, ApiBulkPage>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }

    let endpoint_name = "bulk";

    let mut sorted_ids: Vec<EntryId> = ids.iter().cloned().collect();
    sorted_ids.sort_unstable();

    let total_ids = ids.len();
    let total_batches = (total_ids + config::BULK_BATCH_SIZE - 1) / config::BULK_BATCH_SIZE;

    let mut tasks = JoinSet::new();

    for (i, batch_ids_slice) in sorted_ids.chunks(config::BULK_BATCH_SIZE).enumerate() {
        let batch_ids: Vec<String> = batch_ids_slice.iter().map(ToString::to_string).collect();
        let params = HashMap::from([("str_entry_page_ids".to_string(), batch_ids.join(","))]);
        let client_clone = client.clone();
        let lang_clone = lang.to_string();
        let sem_clone = bulk_sem.clone();
        let batch_num = i + 1;
        let log_ctx_prefix_clone = log_ctx_prefix.to_string();

        tasks.spawn(async move {
            let batch_ctx = format!(
                "{} [{}] Batch {}/{}",
                log_ctx_prefix_clone, lang_clone, batch_num, total_batches
            );

            let permit = utils::acquire_semaphore(&sem_clone, "Bulk Fetch Batch").await?;
            let result = client_clone
                .fetch::<ApiBulkResponse>(
                    Method::GET,
                    endpoint_name,
                    &lang_clone,
                    Some(&params),
                    None,
                )
                .await;
            drop(permit);

            match result {
                Ok(data) => Ok((batch_num, data.entry_pages)),
                Err(AppError::ApiError {
                    retcode: 100010, ..
                }) => {
                    log(
                        LogLevel::Warning,
                        &format!("{} - Batch returned 'Not Found' (100010).", batch_ctx),
                    );
                    Ok((batch_num, Vec::new()))
                }
                Err(e) => {
                    log(
                        LogLevel::Warning,
                        &format!("{} - Batch FAILED: {:?}", batch_ctx, e),
                    );
                    Err(e)
                }
            }
        });
    }

    let mut combined_results = HashMap::with_capacity(total_ids);
    let mut failed_batches = 0;
    let mut successful_fetches = 0;

    while let Some(join_result) = tasks.join_next().await {
        match join_result {
            Ok(Ok((_batch_num, batch_pages))) => {
                successful_fetches += batch_pages.len();
                for page in batch_pages {
                    combined_results.insert(page.id, page);
                }
            }
            Ok(Err(_)) => {
                failed_batches += 1;
            }
            Err(e) => {
                log(
                    LogLevel::Error,
                    &format!(
                        "{} [{}] - Bulk batch task panicked: {}",
                        log_ctx_prefix, lang, e
                    ),
                );
                failed_batches += 1;
            }
        }
    }

    if failed_batches > 0 {
        log(
            LogLevel::Warning,
            &format!(
                "{} [{}] - Completed bulk fetch: {} IDs found, {} batch(es) failed.",
                log_ctx_prefix, lang, successful_fetches, failed_batches
            ),
        );
    }

    Ok(combined_results)
}
