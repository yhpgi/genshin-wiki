use crate::config;
use crate::error::{AppError, AppResult};
use crate::logging::{log, LogLevel};
use clap::Parser;
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Fetches and transforms HoYoWiki data for Android app consumption.",
    long_about = None,
    after_help = format!("Supported languages:\n    all, {}", config::SUPPORTED_LANGS.join(", ")),
    arg_required_else_help = true
)]
pub struct CliArgs {
    #[arg(
        short, long,
        num_args = 1..,
        value_delimiter = ' ',


        help = "Languages to process (e.g., en-us fr-fr ja-jp) or 'all'"
     )]
    languages: Vec<String>,

    #[arg(
        long,
        default_value = config::DEFAULT_OUT_DIR,
         value_name = "DIR_PATH",
         help = "Output directory path"
    )]
    out_dir: String,

    #[arg(
        long,
        value_name = "FILE_PATH",
        help = "Run in test mode using a local JSON file (detail endpoint format)",
        conflicts_with = "languages"
    )]
    test_detail_file: Option<String>,

    #[arg(
        long,
        default_value = "test_output.json",
        value_name = "OUTPUT_FILE",
        help = "Output file name for test mode",
        requires = "test_detail_file"
    )]
    test_output_file: String,
}

impl CliArgs {
    pub fn get_out_dir(&self) -> PathBuf {
        PathBuf::from(&self.out_dir)
    }

    pub fn get_test_detail_file(&self) -> Option<PathBuf> {
        self.test_detail_file.as_deref().map(PathBuf::from)
    }

    pub fn get_test_output_file(&self) -> PathBuf {
        PathBuf::from(&self.test_output_file)
    }

    pub fn get_languages(&self) -> AppResult<Vec<String>> {
        if self.test_detail_file.is_some() {
            if !self.languages.is_empty() {
                log(LogLevel::Warning, "Ignoring specified languages (--languages/-l) because --test-detail-file is active.");
            }
            return Ok(vec!["test-lang".to_string()]);
        }

        if self.languages.is_empty() {
            return Err(AppError::Argument(
                "No languages specified. Use -l or --languages (e.g., 'en-us', 'all').".into(),
            ));
        }

        let inputs: HashSet<String> = self
            .languages
            .iter()
            .map(|s| s.to_lowercase().trim().to_string())
            .collect();

        if inputs.contains("all") {
            log(
                LogLevel::Info,
                "Processing all supported languages requested.",
            );
            let mut sorted_langs: Vec<String> = config::SUPPORTED_LANGS.to_vec();
            sorted_langs.sort_unstable();
            Ok(sorted_langs)
        } else {
            let supported_set: HashSet<String> = config::SUPPORTED_LANGS.iter().cloned().collect();
            let mut valid_langs = Vec::new();
            let mut invalid_langs = Vec::new();

            for lang in inputs {
                if supported_set.contains(&lang) {
                    valid_langs.push(lang);
                } else {
                    invalid_langs.push(lang);
                }
            }

            if !invalid_langs.is_empty() {
                log(
                    LogLevel::Warning,
                    &format!(
                        "Ignoring unsupported language codes: {}",
                        invalid_langs.join(", ")
                    ),
                );
            }

            if valid_langs.is_empty() {
                return Err(AppError::Argument(
                    "No *valid* supported languages specified.".into(),
                ));
            }

            valid_langs.sort_unstable();
            log(
                LogLevel::Info,
                &format!("Processing specified languages: {}", valid_langs.join(", ")),
            );
            Ok(valid_langs)
        }
    }
}
