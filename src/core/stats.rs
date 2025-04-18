use crate::logging::{log, LogLevel};
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Debug, Clone, Default)]
pub struct CategoryStats {
    pub ok: usize,
    pub fail: usize,
    pub skip_or_empty: usize,
    pub total_tasks: usize,
}

impl CategoryStats {
    pub fn add_ok(&mut self) {
        self.ok += 1;
    }
    pub fn add_fail(&mut self) {
        self.fail += 1;
    }
    pub fn add_skip(&mut self) {
        self.skip_or_empty += 1;
    }
    pub fn set_total(&mut self, total: usize) {
        self.total_tasks = total;
    }
    pub fn get_processed(&self) -> usize {
        self.ok + self.fail + self.skip_or_empty
    }
}

pub type RunStats = BTreeMap<String, CategoryStats>;

pub fn initialize_stats() -> RunStats {
    let mut stats = BTreeMap::new();

    stats.insert("Navigation".to_string(), Default::default());
    stats.insert("List Fetch".to_string(), Default::default());
    stats.insert("Detail Fetch".to_string(), Default::default());
    stats.insert("Calendar Fetch".to_string(), Default::default());
    stats.insert("Bulk Primary".to_string(), Default::default());
    stats.insert("Bulk Fallback".to_string(), Default::default());

    stats.insert("Save Files".to_string(), Default::default());
    stats
}

pub fn print_summary(stats: &RunStats, languages: &[String], duration: Duration) {
    let sep = "=".repeat(60);
    let title = format!("Run Summary ({} Languages)", languages.len());
    println!("\n{}\n{:^60}\n{}", sep, title, sep);
    if !languages.is_empty() {
        println!("Languages:         {}", languages.join(", "));
    }
    println!("Total Run Time:    {:.3?}", duration);
    println!("{}", "-".repeat(60));

    println!(
        "{:<17} {:<8} {:<12} {:<8} {:<8}",
        "Category", "OK", "Skip/Empty", "Fail", "Total"
    );
    println!("{}", "-".repeat(60));

    let mut grand_total_fetch_tasks = 0;
    let mut grand_total_fetch_ok = 0;
    let mut grand_total_fetch_empty = 0;
    let mut grand_total_fetch_fail = 0;

    let categories_order = [
        "Navigation",
        "List Fetch",
        "Detail Fetch",
        "Calendar Fetch",
        "Bulk Primary",
        "Bulk Fallback",
        "Save Files",
    ];

    for &cat_name in &categories_order {
        if let Some(s) = stats.get(cat_name) {
            println!(
                "{:<17} {:<8} {:<12} {:<8} {:<8}",
                cat_name, s.ok, s.skip_or_empty, s.fail, s.total_tasks
            );

            if [
                "Navigation",
                "List Fetch",
                "Detail Fetch",
                "Calendar Fetch",
                "Bulk Primary",
                "Bulk Fallback",
            ]
            .contains(&cat_name)
            {
                grand_total_fetch_tasks += s.total_tasks;
                grand_total_fetch_ok += s.ok;
                grand_total_fetch_empty += s.skip_or_empty;
                grand_total_fetch_fail += s.fail;
            }
        }
    }

    if grand_total_fetch_tasks > 0 {
        println!("{}", "-".repeat(60));
        println!(
            "{:<17} {:<8} {:<12} {:<8} {:<8}",
            "FETCH TOTALS",
            grand_total_fetch_ok,
            grand_total_fetch_empty,
            grand_total_fetch_fail,
            grand_total_fetch_tasks
        );
    }

    println!("{}", sep);

    log_overall_status(stats, grand_total_fetch_fail, languages.is_empty());

    let end_ts_str = chrono::Utc::now()
        .format("%Y-%m-%d %H:%M:%S %Z")
        .to_string();
    log(
        LogLevel::Step,
        &format!("--- Run Finished at {} ---", end_ts_str),
    );
}

fn log_overall_status(stats: &RunStats, total_fetch_failures: usize, no_languages_processed: bool) {
    let save_failures = stats.get("Save Files").map_or(0, |s| s.fail);

    if no_languages_processed {
        log(
            LogLevel::Warning,
            "Run completed, but no target languages were specified or processed successfully.",
        );
    } else if total_fetch_failures > 0 || save_failures > 0 {
        log(LogLevel::Error, &format!("Run completed with errors: {} fetch task(s) and {} save task(s) failed. Check logs.", total_fetch_failures, save_failures));
    } else {
        log(LogLevel::Success, "Run completed successfully.");
    }
}

pub fn determine_exit_code(stats: &RunStats) -> i32 {
    let fetch_failures = [
        "Navigation",
        "List Fetch",
        "Detail Fetch",
        "Calendar Fetch",
        "Bulk Primary",
        "Bulk Fallback",
    ]
    .iter()
    .any(|&cat| stats.get(cat).map_or(false, |s| s.fail > 0));

    let save_failures = stats.get("Save Files").map_or(0, |s| s.fail) > 0;

    if fetch_failures || save_failures {
        1
    } else {
        0
    }
}
