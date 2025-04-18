use crate::api::model::{ApiDetailResponse, ApiWrapper};
use crate::error::{AppError, AppResult};
use crate::io;
use crate::logging::{log, LogLevel};
use crate::transform;
use crate::transform::bulk::BulkStore;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;

pub async fn test_detail_transform(input_path: &Path, output_path: PathBuf) -> AppResult<()> {
    log(
        LogLevel::Info,
        &format!("--- Running Detail Transform Test ---"),
    );
    log(
        LogLevel::Info,
        &format!("Input file: {}", input_path.display()),
    );
    log(
        LogLevel::Info,
        &format!("Output file: {}", output_path.display()),
    );

    let json_content = fs::read_to_string(input_path)
        .await
        .map_err(AppError::from)?;

    let wrapper: ApiWrapper<ApiDetailResponse> =
        serde_json::from_str(&json_content).map_err(AppError::from)?;

    if wrapper.retcode != 0 {
        return Err(AppError::api_error(
            wrapper.retcode,
            wrapper.message,
            "local test file",
            "test-lang",
        ));
    }

    let detail_response = wrapper
        .data
        .ok_or_else(|| AppError::response_invalid("Missing 'data' field", "test", "test-lang"))?;

    let raw_page = detail_response.page;

    let bulk_store = Arc::new(BulkStore::default());
    let lang = "test-lang";

    log(LogLevel::Info, "Starting transformation...");

    let transform_result =
        transform::detail::transform_detail_page(raw_page, bulk_store, lang).await;
    log(LogLevel::Info, "Transformation finished.");

    match transform_result {
        Ok(Some(output_page)) => {
            log(LogLevel::Success, "Transformation successful.");
            let log_ctx = format!("Test Detail File (Entry: {})", output_page.id);

            match io::save_json(output_path.clone(), output_page, log_ctx).await {
                Ok(true) => {
                    log(
                        LogLevel::Success,
                        &format!(
                            "Successfully saved transformed data to {}",
                            output_path.display()
                        ),
                    );
                    Ok(())
                }

                Ok(false) => {
                    log(
                        LogLevel::Error,
                        &format!(
                            "Failed to save transformed data to {}",
                            output_path.display()
                        ),
                    );
                    Err(AppError::Io("Failed to save JSON".to_string()))
                }
                Err(e) => {
                    log(LogLevel::Error, &format!("Error during saving: {:?}", e));
                    Err(e)
                }
            }
        }
        Ok(None) => {
            log(LogLevel::Warning, "Transformation resulted in no output (likely filtered out or empty). No file generated.");
            Ok(())
        }
        Err(e) => {
            log(LogLevel::Error, &format!("Transformation failed: {:?}", e));
            Err(e)
        }
    }
}
