#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod overlay;
mod worker;

use std::{
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard},
};
use ropalolyst_core::{default_log_path, HistoryRecord, PhaseSplit, RunStore};
use serde::Serialize;
use tauri::{Manager, State};

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|error| error.into_inner())
}

#[derive(Clone, Serialize, Default)]
struct Status {
    mode: String,
    path: String,
    error: String,
    offset: u64,
    revision: u64,
    active: bool,
    elapsed_seconds: Option<f64>,
    splits: Vec<PhaseSplit>,
}

struct Shared {
    path: Mutex<PathBuf>,
    status: Mutex<Status>,
    store: Mutex<RunStore>,
    overlay: Mutex<overlay::Runtime>,
}

fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[tauri::command]
fn get_status(shared: State<'_, Arc<Shared>>) -> Status {
    lock(&shared.status).clone()
}

#[tauri::command]
fn get_runs(shared: State<'_, Arc<Shared>>) -> Result<Vec<HistoryRecord>, String> {
    lock(&shared.store).history().map_err(err)
}

#[tauri::command]
fn rename_run(shared: State<'_, Arc<Shared>>, id: i64, name: String) -> Result<(), String> {
    lock(&shared.store).rename(id, &name).map_err(err)?;
    lock(&shared.status).revision += 1;
    Ok(())
}

#[tauri::command]
fn favorite_run(shared: State<'_, Arc<Shared>>, id: i64) -> Result<(), String> {
    lock(&shared.store).toggle_favorite(id).map_err(err)?;
    lock(&shared.status).revision += 1;
    Ok(())
}

#[tauri::command]
fn delete_run(shared: State<'_, Arc<Shared>>, id: i64) -> Result<(), String> {
    lock(&shared.store).delete(id).map_err(err)?;
    lock(&shared.status).revision += 1;
    Ok(())
}

#[tauri::command]
fn set_log_path(shared: State<'_, Arc<Shared>>, path: String) -> Result<(), String> {
    let path = if path.trim().is_empty() {
        default_log_path().ok_or("未找到 LOCALAPPDATA 环境变量")?
    } else {
        PathBuf::from(path.trim())
    };
    if !path.is_absolute() || path.file_name().is_none() {
        return Err("请输入 EE.log 的绝对路径".into());
    }
    lock(&shared.store)
        .set_setting("log_path", &path.to_string_lossy())
        .map_err(err)?;
    *lock(&shared.path) = path;
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // Keep the pre-desktop database in place, preserving all existing race records.
            let database = match default_log_path() {
                Some(path) => path.with_file_name("ropalolyst-analytics.sqlite3"),
                None => {
                    let dir = app.path().app_local_data_dir()?;
                    std::fs::create_dir_all(&dir)?;
                    dir.join("ropalolyst-analytics.sqlite3")
                }
            };
            let store = RunStore::open(&database)?;
            let path = store
                .get_setting("log_path")?
                .map(PathBuf::from)
                .or_else(default_log_path)
                .unwrap_or_default();
            let config = store
                .get_setting("overlay_config")?
                .and_then(|raw| serde_json::from_str(&raw).ok())
                .unwrap_or_default();
            let shared = Arc::new(Shared {
                path: Mutex::new(path.clone()),
                status: Mutex::new(Status {
                    mode: "waiting".into(),
                    path: path.to_string_lossy().to_string(),
                    ..Status::default()
                }),
                store: Mutex::new(store),
                overlay: Mutex::new(overlay::Runtime::new(config)),
            });
            app.manage(shared.clone());
            if let Err(error) = overlay::initialize(app.handle(), &shared) {
                eprintln!("Overlay: {error}");
            }
            let app_handle = app.handle().clone();
            std::thread::Builder::new()
                .name("ee-log-reader".into())
                .spawn(move || worker::run(shared, app_handle))?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    window.app_handle().exit(0);
                } else if window.label() == "overlay" {
                    api.prevent_close();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_runs,
            rename_run,
            favorite_run,
            delete_run,
            set_log_path,
            overlay::get_overlay_settings,
            overlay::get_overlay_frame,
            overlay::set_overlay_settings,
            overlay::edit_overlay,
            overlay::reset_overlay,
            overlay::drag_overlay,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Ropalolyst Analytics");
}
