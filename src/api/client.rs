use crate::api::model::ApiWrapper;
use crate::config;
use crate::error::{AppError, AppResult};
use crate::logging::{log, LogLevel};
use bytes::Bytes;
use reqwest::{header::HeaderValue, Client, Method, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Clone)]
pub struct ApiClient {
    client: Client,
}

impl ApiClient {
    pub fn new() -> AppResult<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config::HTTP_TIMEOUT_SECONDS))
            .connect_timeout(Duration::from_secs(config::HTTP_CONNECT_TIMEOUT))
            .build()
            .map_err(AppError::from)?;
        Ok(ApiClient { client })
    }

    pub async fn fetch<T>(
        &self,
        method: Method,
        endpoint_key: &'static str,
        lang: &str,
        params: Option<&HashMap<String, String>>,
        payload: Option<&Value>,
    ) -> AppResult<T>
    where
        T: DeserializeOwned,
    {
        let url = config::API_ENDPOINTS.get(endpoint_key).ok_or_else(|| {
            AppError::ConfigError(format!("Invalid endpoint key: {}", endpoint_key))
        })?;

        let bytes = self
            .fetch_internal(method, url, lang, params, payload, endpoint_key)
            .await?;

        let wrapper: ApiWrapper<T> = serde_json::from_slice(&bytes).map_err(|e| {
            let snippet_len = bytes.len().min(200);
            let snippet = String::from_utf8_lossy(&bytes[..snippet_len]);
            log(
                LogLevel::Error,
                &format!(
                    "Fail parse API wrapper for {} [{}] Type {}: {}. Snippet: '{}'",
                    endpoint_key,
                    lang,
                    std::any::type_name::<T>(),
                    e,
                    snippet
                ),
            );

            AppError::from(e)
        })?;

        if wrapper.retcode != 0 {
            if wrapper.retcode == 100010 {
                return Err(AppError::api_error(100010, "Not Found", endpoint_key, lang));
            }

            return Err(AppError::api_error(
                wrapper.retcode,
                wrapper.message,
                endpoint_key,
                lang,
            ));
        }

        wrapper.data.ok_or_else(|| {
            AppError::response_invalid(
                format!(
                    "Missing 'data' field in successful response ({})",
                    endpoint_key
                ),
                endpoint_key,
                lang,
            )
        })
    }

    async fn fetch_internal(
        &self,
        method: Method,
        url: &str,
        lang: &str,
        params: Option<&HashMap<String, String>>,
        json_payload: Option<&Value>,
        endpoint_key: &'static str,
    ) -> AppResult<Bytes> {
        let mut last_error: Option<AppError> = None;

        for attempt in 0..=config::MAX_RETRIES {
            let mut headers = config::BASE_UA_HEADERS.clone();
            headers.insert(
                "x-rpc-language",
                HeaderValue::from_str(lang).map_err(|_| {
                    AppError::ConfigError(format!("Invalid lang code for header: {}", lang))
                })?,
            );

            let mut request_builder = self.client.request(method.clone(), url).headers(headers);
            if let Some(p) = params {
                request_builder = request_builder.query(p);
            }
            if let Some(payload) = json_payload {
                request_builder = request_builder.json(payload);
            }

            let url_tag = url.split('/').last().unwrap_or("unknown_endpoint");
            let log_prefix = format!(
                "API Req [{}] {} {} (Try {})",
                lang,
                method,
                url_tag,
                attempt + 1
            );

            match request_builder.send().await {
                Ok(resp) => {
                    let status = resp.status();

                    if status.is_success() {
                        return resp.bytes().await.map_err(|e| {
                            log(
                                LogLevel::Warning,
                                &format!(
                                    "{} - Error reading success response body: {}",
                                    log_prefix, e
                                ),
                            );
                            AppError::from(e)
                        });
                    } else {
                        let error = self
                            .handle_http_error(resp, status, endpoint_key, lang, &log_prefix)
                            .await;
                        let should_stop_retrying = matches!(
                            error,
                            AppError::ApiError {
                                retcode: 100010,
                                ..
                            }
                        );
                        last_error = Some(error);

                        if should_stop_retrying {
                            return Err(last_error.unwrap());
                        }

                        log(
                            LogLevel::Warning,
                            &format!("{} Failed: {:?}", log_prefix, last_error),
                        );
                    }
                }
                Err(e) => {
                    let context_str = if e.is_timeout() {
                        "Timeout"
                    } else if e.is_connect() {
                        "Connection"
                    } else {
                        "Request"
                    };
                    let error_message = format!("{} Error: {}", context_str, e);
                    let app_error = if e.is_timeout() {
                        AppError::Timeout(format!("{} {}", log_prefix, error_message))
                    } else {
                        AppError::from(e)
                    };
                    log(
                        LogLevel::Warning,
                        &format!("{} {}", log_prefix, error_message),
                    );
                    last_error = Some(app_error);
                }
            }

            if attempt < config::MAX_RETRIES {
                let delay_secs = config::RETRY_DELAY_BASE_SECS * (2.0_f32.powi(attempt as i32));
                sleep(Duration::from_secs_f32(delay_secs)).await;
            }
        }

        Err(last_error.unwrap_or_else(|| {
            AppError::Unexpected(format!(
                "Request failed after {} retries for {} [{}]",
                config::MAX_RETRIES + 1,
                url,
                lang
            ))
        }))
    }

    async fn handle_http_error(
        &self,
        resp: Response,
        status: StatusCode,
        endpoint_key: &'static str,
        lang: &str,
        log_prefix: &str,
    ) -> AppError {
        let retcode = status.as_u16() as i64;

        if status == StatusCode::NOT_FOUND
            || status == StatusCode::BAD_REQUEST
            || status == StatusCode::FORBIDDEN
        {
            log(
                LogLevel::Info,
                &format!(
                    "{} Request resulted in {}. Mapping to 'Not Found'.",
                    log_prefix, status
                ),
            );
            return AppError::api_error(
                100010,
                format!("Resource not found or bad request ({})", status),
                endpoint_key,
                lang,
            );
        }
        if status == StatusCode::PAYLOAD_TOO_LARGE {
            log(
                LogLevel::Error,
                &format!("{} - Payload Too Large (413)", log_prefix),
            );
            return AppError::api_error(413, "Payload Too Large", endpoint_key, lang);
        }

        let resp_text = resp
            .text()
            .await
            .unwrap_or_else(|_| "Failed to read error body".to_string());
        let error_message = format!(
            "HTTP {} ({}). Body: {}...",
            status,
            status.canonical_reason().unwrap_or("Unknown Status"),
            resp_text.chars().take(150).collect::<String>()
        );

        AppError::api_error(retcode, error_message, endpoint_key, lang)
    }
}
