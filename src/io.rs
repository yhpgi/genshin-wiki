use crate::config;
use crate::error::{AppError, AppResult};
use crate::logging::{log, LogLevel};
use crate::utils;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;

pub fn clean_filename<S: AsRef<str>>(name: S) -> String {
    let name_ref = name.as_ref().trim();
    if name_ref.is_empty() {
        return "invalid_empty_name".to_string();
    }

    let cleaned = config::FORBIDDEN_CHARS_RE.replace_all(name_ref, "_");
    let cleaned = config::WHITESPACE_RE.replace_all(&cleaned, "_");

    let cleaned = cleaned.trim_matches('_').to_lowercase();

    if cleaned.is_empty() {
        "invalid_or_empty_name".to_string()
    } else {
        cleaned
    }
}

pub async fn ensure_output_directories(base_dir: &Path) -> AppResult<()> {
    log(
        LogLevel::Info,
        &format!(
            "Ensuring base output directories exist under: {}",
            base_dir.display()
        ),
    );

    fs::create_dir_all(base_dir)
        .await
        .map_err(|e| map_io_error(e, base_dir))?;

    let subdirs = ["navigation", "list", "detail", "calendar"];
    for subdir in subdirs {
        let dir_path = base_dir.join(subdir);
        fs::create_dir_all(&dir_path)
            .await
            .map_err(|e| map_io_error(e, &dir_path))?;
    }
    Ok(())
}

fn map_io_error(error: std::io::Error, path: &Path) -> AppError {
    AppError::Io(format!("I/O error at path '{}': {}", path.display(), error))
}

async fn write_file_async(fpath: &Path, data: &[u8]) -> AppResult<()> {
    let mut file = File::create(fpath)
        .await
        .map_err(|e| map_io_error(e, fpath))?;
    file.write_all(data)
        .await
        .map_err(|e| map_io_error(e, fpath))?;

    Ok(())
}

pub async fn save_json<T>(fpath: PathBuf, data: T, log_ctx: String) -> AppResult<bool>
where
    T: Serialize + Send + Sync + 'static,
{
    let json_string_result =
        utils::run_blocking(move || serde_json::to_string_pretty(&data).map_err(AppError::from)).await;

    match json_string_result {
        Ok(json_string) => {
            let json_bytes = json_string.into_bytes();
            match write_file_async(&fpath, &json_bytes).await {
                Ok(_) => Ok(true),
                Err(e) => {
                    log(
                        LogLevel::Error,
                        &format!(
                            "Save JSON ({}) FAIL - Write Error: {}. File: '{}'",
                            log_ctx,
                            e,
                            fpath.display()
                        ),
                    );

                    if fs::try_exists(&fpath).await.unwrap_or(false) {
                        let _ = fs::remove_file(&fpath).await;
                    }

                    Err(e)
                }
            }
        }
        Err(e) => {
            log(
                LogLevel::Error,
                &format!(
                    "Save JSON ({}) FAIL - Serialize/Task Error: {}. File: '{}'",
                    log_ctx,
                    e,
                    fpath.display()
                ),
            );
            Err(e)
        }
    }
}