use std::num::NonZeroUsize;

use rocket::serde::json::Json;
use rocket::{figment::Figment, http::Status, Rocket, State};
use rocket::response::Redirect;
use rocket_dyn_templates::Template;
use serde::Deserialize;
use systemstat::{Duration, Platform, System};

mod stats;
use stats::*;

mod stats_history;
use stats_history::*;

mod dashboard_context;
use dashboard_context::*;

mod error_context;
use error_context::*;

#[macro_use]
extern crate rocket;

const CPU_LOAD_SAMPLE_DURATION: Duration = Duration::from_millis(500);
const DEFAULT_DARK_MODE: bool = true;

const RECENT_HISTORY_SIZE_CONFIG_KEY: &str = "recent_history_size";
const DEFAULT_RECENT_HISTORY_SIZE: usize = 180;

const CONSOLIDATION_LIMIT_CONFIG_KEY: &str = "consolidation_limit";
const DEFAULT_CONSOLIDATION_LIMIT: usize = 20;

const UPDATE_FREQUENCY_CONFIG_KEY: &str = "update_frequency_seconds";
const DEFAULT_UPDATE_FREQUENCY_SECONDS: u64 = 3;

const PERSIST_HISTORY_TOGGLE_CONFIG_KEY: &str = "persist_history";
const DEFAULT_PERSIST_HISTORY_TOGGLE: bool = true;

const HISTORY_FILES_DIRECTORY_CONFIG_KEY: &str = "history_files_directory";
const DEFAULT_HISTORY_FILES_DIRECTORY: &str = "./stats_history";

const HISTORY_FILES_DIRECTORY_MAX_SIZE_CONFIG_KEY: &str = "history_files_max_size_bytes";
const DEFAULT_HISTORY_FILES_DIRECTORY_MAX_SIZE_BYTES: u64 = 2_000_000; // 2MB

/// 获取最新的系统统计信息
#[get("/stats")]
fn get_all_stats(stats_history: &State<UpdatingStatsHistory>) -> Result<Json<AllStats>, Status> {
    match stats_history
        .stats_history
        .lock()
        .unwrap()
        .get_most_recent_stats()
    {
        Some(x) => Ok(Json((*x).clone())),
        None => Err(Status::InternalServerError),
    }
}

/// 获取一般统计信息
#[get("/stats/general")]
fn get_general_stats(
    stats_history: &State<UpdatingStatsHistory>,
) -> Result<Json<GeneralStats>, Status> {
    match stats_history
        .stats_history
        .lock()
        .unwrap()
        .get_most_recent_stats()
    {
        Some(x) => Ok(Json((*x).general.clone())),
        None => Err(Status::InternalServerError),
    }
}

/// 获取 CPU 统计信息
#[get("/stats/cpu")]
fn get_cpu_stats(stats_history: &State<UpdatingStatsHistory>) -> Result<Json<CpuStats>, Status> {
    match stats_history
        .stats_history
        .lock()
        .unwrap()
        .get_most_recent_stats()
    {
        Some(x) => Ok(Json((*x).cpu.clone())),
        None => Err(Status::InternalServerError),
    }
}

/// 获取内存统计信息
#[get("/stats/memory")]
fn get_memory_stats() -> Result<Json<MemoryStats>, Status> {
    match MemoryStats::from(&System::new()) {
        Some(x) => Ok(Json(x)),
        None => Err(Status::InternalServerError),
    }
}

/// 获取文件系统统计信息
#[get("/stats/filesystems")]
fn get_filesystem_stats() -> Result<Json<Vec<MountStats>>, Status> {
    match MountStats::from(&System::new()) {
        Some(x) => Ok(Json(x)),
        None => Err(Status::InternalServerError),
    }
}

/// 获取网络统计信息
#[get("/stats/network")]
fn get_network_stats() -> Json<NetworkStats> {
    Json(NetworkStats::from(&System::new()))
}

/// 首页 - 转发到查看仪表板
#[get("/")]
fn index() -> Redirect {
    Redirect::to(rocket::uri!(dashboard(Some(true))))
}

/// 查看仪表板
#[get("/dashboard?<dark>")]
fn dashboard(stats_history: &State<UpdatingStatsHistory>, dark: Option<bool>) -> Template {
    let context = DashboardContext::from_history(
        &stats_history.stats_history.lock().unwrap(),
        dark.unwrap_or(DEFAULT_DARK_MODE),
    );
    Template::render("dashboard", &context)
}

/// 用于查看持久统计信息仪表板（历史信息）
#[get("/dashboard/history?<dark>")]
fn history_dashboard(
    history_persistence_config: &State<HistoryPersistenceConfig>,
    dark: Option<bool>,
) -> Result<Template, Status> {
    match history_persistence_config.inner() {
        HistoryPersistenceConfig::Enabled { dir, size_limit: _ } => {
            let history = match StatsHistory::load_from(dir) {
                Ok(x) => x,
                Err(e) => {
                    println!("Error loading persisted stats from {:?}: {}", dir, e);
                    return Err(Status::InternalServerError);
                }
            };
            let context =
                DashboardContext::from_history(&history, dark.unwrap_or(DEFAULT_DARK_MODE));
            Ok(Template::render("dashboard", &context))
        }
        HistoryPersistenceConfig::Disabled => Ok(Template::render(
            "error",
            &ErrorContext {
                title: "Stats History".to_string(),
                message: "Stats history persistence is disabled.".to_string(),
            },
        )),
    }
}

#[launch]
fn rocket() -> Rocket<rocket::Build> {
    let mut rocket = rocket::build()
        .mount(
            "/",
            rocket::routes![
                get_all_stats,
                get_general_stats,
                get_cpu_stats,
                get_memory_stats,
                get_filesystem_stats,
                get_network_stats,
                index,
                dashboard,
                history_dashboard,
            ],
        )
        .attach(Template::fairing());

    let config = rocket.figment();

    let update_frequency_secs = get_config_value(
        config,
        UPDATE_FREQUENCY_CONFIG_KEY,
        DEFAULT_UPDATE_FREQUENCY_SECONDS,
    );

    let recent_history_size = get_config_value(
        config,
        RECENT_HISTORY_SIZE_CONFIG_KEY,
        DEFAULT_RECENT_HISTORY_SIZE,
    );

    let consolidation_limit = get_config_value(
        config,
        CONSOLIDATION_LIMIT_CONFIG_KEY,
        DEFAULT_CONSOLIDATION_LIMIT,
    );

    let history_persistence_enabled = get_config_value(
        config,
        PERSIST_HISTORY_TOGGLE_CONFIG_KEY,
        DEFAULT_PERSIST_HISTORY_TOGGLE,
    );
    let persistence_config = if history_persistence_enabled {
        let history_files_dir = get_config_value(
            config,
            HISTORY_FILES_DIRECTORY_CONFIG_KEY,
            DEFAULT_HISTORY_FILES_DIRECTORY.to_string(),
        );
        let history_files_dir_max_size = get_config_value(
            config,
            HISTORY_FILES_DIRECTORY_MAX_SIZE_CONFIG_KEY,
            DEFAULT_HISTORY_FILES_DIRECTORY_MAX_SIZE_BYTES,
        );
        HistoryPersistenceConfig::Enabled {
            dir: history_files_dir.into(),
            size_limit: history_files_dir_max_size,
        }
    } else {
        HistoryPersistenceConfig::Disabled
    };

    rocket = rocket
        .manage(persistence_config.clone())
        .manage(UpdatingStatsHistory::new(
            System::new(),
            CPU_LOAD_SAMPLE_DURATION,
            Duration::from_secs(update_frequency_secs),
            NonZeroUsize::new(recent_history_size).unwrap(),
            NonZeroUsize::new(consolidation_limit).unwrap(),
            persistence_config,
        ));

    rocket
}

/// 从提供的配置中获取一个值，如果未找到，则返回默认值。
fn get_config_value<'a, T>(config: &Figment, key: &str, default: T) -> T
where
    T: Deserialize<'a> + std::fmt::Debug,
{
    match config.extract_inner(key) {
        Ok(x) => {
            println!("为 {} 使用配置值 {:?}", key, x);
            x
        }
        Err(e) => {
            println!("为 {} 使用默认值 {:?} ({})", key, default, e);
            default
        }
    }
}
