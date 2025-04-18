use clap::{CommandFactory, Parser};
use std::process::ExitCode;
use std::sync::Arc;
use tokio::runtime::Builder;
use wiki_update::cli::CliArgs;
use wiki_update::core::processor;
use wiki_update::error::{AppError, AppResult};
use wiki_update::logging::{log, setup_logging, LogLevel};
use wiki_update::testing;

fn main() -> ExitCode {
    setup_logging();

    let cli_args = match CliArgs::try_parse() {
        Ok(args) => args,
        Err(e) => {
            log(LogLevel::Error, &format!("CLI Argument Error: {}", e));
            let _ = CliArgs::command().print_help();
            return ExitCode::from(2);
        }
    };

    let runtime = match Builder::new_multi_thread()
        .enable_all()
        .thread_name("wiki-worker")
        .worker_threads(num_cpus::get())
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            log(
                LogLevel::Error,
                &format!("FATAL: Failed to build Tokio runtime: {}", e),
            );
            return ExitCode::FAILURE;
        }
    };

    let cli_args_arc = Arc::new(cli_args);

    let main_result: AppResult<i32> = runtime.block_on(async {
        let args = cli_args_arc;

        if let Some(test_file_path) = args.get_test_detail_file() {
            let output_path = args.get_test_output_file();

            if !test_file_path.exists() {
                log(
                    LogLevel::Error,
                    &format!("Test input file not found: {}", test_file_path.display()),
                );
                return Err(AppError::Argument("Test input file not found.".to_string()));
            }

            match testing::test_detail_transform(&test_file_path, output_path).await {
                Ok(_) => Ok(0),
                Err(e) => {
                    log(LogLevel::Error, &format!("Test mode failed: {:?}", e));
                    Ok(1)
                }
            }
        } else {
            let target_langs = match args.get_languages() {
                Ok(langs) => langs,
                Err(e) => {
                    log(LogLevel::Error, &e.to_string());
                    let _ = CliArgs::command().print_help();
                    return Err(e);
                }
            };

            let out_dir = args.get_out_dir();

            processor::run(target_langs, out_dir).await
        }
    });

    match main_result {
        Ok(exit_code) => ExitCode::from(exit_code as u8),
        Err(e) => {
            if !matches!(e, AppError::Argument(_)) {
                log(LogLevel::Error, &format!("FATAL UNEXPECTED ERROR: {:?}", e));
            }
            ExitCode::FAILURE
        }
    }
}
