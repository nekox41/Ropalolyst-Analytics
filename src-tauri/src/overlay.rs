
use crate::{lock, Shared};
use serde::{Deserialize, Serialize};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tauri::{AppHandle, LogicalSize, Manager, PhysicalPosition, State, WebviewWindow};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub foreground_only: bool,
    pub auto_hide: bool,
    pub opacity: u8,
    pub scale: u8,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub width: Option<f64>,
    pub height: Option<f64>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            foreground_only: true,
            auto_hide: true,
            opacity: 87,
            scale: 100,
            x: None,
            y: None,
            width: None,
            height: None,
        }
    }
}

impl Config {
    pub fn normalize(&mut self) {
        self.opacity = self.opacity.clamp(20, 100);
        self.scale = self.scale.clamp(50, 200);
        self.width = self.width.filter(|n| n.is_finite()).map(|n| n.clamp(230.0, 1600.0));
        self.height = self.height.filter(|n| n.is_finite()).map(|n| n.clamp(70.0, 500.0));
    }

    pub fn dimensions(&self) -> (f64, f64) {
        let factor = f64::from(self.scale) / 100.0;
        (self.width.unwrap_or(360.0 * factor), self.height.unwrap_or(100.0 * factor))
    }
}

#[derive(Serialize, Clone)]
pub struct Settings {
    pub config: Config,
    pub editing: bool,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct Frame {
    pub settings: Settings,
    pub elapsed_seconds: Option<f64>,
    pub finished: bool,
}

pub struct Runtime {
    pub config: Config,
    editing: bool,
    ready: bool,
    shown: bool,
    error: Option<String>,
    sample: Option<(String, f64, Instant)>,
    frozen: Option<(f64, Instant)>,
}

impl Runtime {
    pub fn new(mut config: Config) -> Self {
        config.normalize();
        Self { config, editing: false, ready: false, shown: false, error: None, sample: None, frozen: None }
    }
    fn settings(&self) -> Settings {
        Settings { config: self.config.clone(), editing: self.editing, error: self.error.clone() }
    }
    fn frame(&self) -> Frame {
        let value = self.sample.as_ref().map(|(_, elapsed, instant)| elapsed + instant.elapsed().as_secs_f64());
        let finished = self.frozen.is_some();
        Frame {
            settings: self.settings(),
            elapsed_seconds: self.frozen.as_ref().map(|(elapsed, _)| *elapsed).or(value),
            finished,
        }
    }
    fn clear(&mut self) {
        self.sample = None;
        self.frozen = None;
    }
    pub fn live(&mut self, key: String, elapsed_seconds: f64) {
        if self.sample.as_ref().is_none_or(|(existing, _, _)| existing != &key) {
            self.sample = Some((key, elapsed_seconds.max(0.0), Instant::now()));
        }
        self.frozen = None;
    }
    pub fn finished(&mut self, elapsed_seconds: f64) {
        self.sample = None;
        self.frozen = Some((elapsed_seconds.max(0.0), Instant::now()));
    }
    pub fn tick(&mut self) {
        if self.frozen.as_ref().is_some_and(|(_, at)| at.elapsed() > Duration::from_secs(5)) {
            self.clear();
        }
    }
}

fn err(value: impl std::fmt::Display) -> String { value.to_string() }
fn window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window("overlay").ok_or_else(|| "Overlay 窗口不可用".into())
}

#[cfg(windows)]
fn game_foreground() -> bool {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::Threading::{
            OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
        },
        UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId},
    };
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() { return false; }
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() { return false; }
        let mut buf = [0u16; 1024];
        let mut size = buf.len() as u32;
        let result = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut size);
        CloseHandle(handle);
        if result == 0 { return false; }
        let path = String::from_utf16_lossy(&buf[..size as usize]);
        path.rsplit('\\').next().is_some_and(|name| {
            name.eq_ignore_ascii_case("Warframe.x64.exe")
                || name.eq_ignore_ascii_case("Warframe.exe")
        })
    }
}
#[cfg(not(windows))]
fn game_foreground() -> bool { true }

fn apply_geometry(win: &WebviewWindow, config: &Config) -> Result<(), String> {
    let (width, height) = config.dimensions();
    win.set_min_size(Some(LogicalSize::new(230.0, 70.0))).map_err(err)?;
    win.set_size(LogicalSize::new(width, height)).map_err(err)?;
    let monitors = win.available_monitors().map_err(err)?;
    if let (Some(x), Some(y)) = (config.x, config.y) {
        if monitors.iter().any(|m| {
            let p = m.position();
            let s = m.size();
            i64::from(x) >= i64::from(p.x)
                && i64::from(y) >= i64::from(p.y)
                && i64::from(x) + 60 < i64::from(p.x) + i64::from(s.width)
                && i64::from(y) + 40 < i64::from(p.y) + i64::from(s.height)
        }) {
            return win.set_position(PhysicalPosition::new(x, y)).map_err(err);
        }
    }
    if let Some(monitor) = win.primary_monitor().map_err(err)? {
        let p = monitor.position();
        let s = monitor.size();
        let w = win.outer_size().map_err(err)?.width;
        let x = i64::from(p.x) + (i64::from(s.width) - i64::from(w)).max(0) / 2;
        let y = i64::from(p.y) + (20.0 * monitor.scale_factor()) as i64;
        win.set_position(PhysicalPosition::new(x as i32, y as i32)).map_err(err)?;
    }
    Ok(())
}

pub fn initialize(app: &AppHandle, shared: &Arc<Shared>) -> Result<(), String> {
    let mut runtime = lock(&shared.overlay);
    let result = (|| {
        let win = window(app)?;
        win.set_ignore_cursor_events(true).map_err(err)?;
        win.set_focusable(false).map_err(err)?;
        apply_geometry(&win, &runtime.config)
    })();
    runtime.ready = result.is_ok();
    runtime.error = result.as_ref().err().cloned();
    result
}

pub fn sync(app: &AppHandle, runtime: &mut Runtime) -> Result<(), String> {
    runtime.tick();
    if !runtime.ready { return Ok(()); }
    let active = runtime.sample.is_some() || runtime.frozen.is_some();
    let show = runtime.editing || (
        runtime.config.enabled
            && (active || !runtime.config.auto_hide)
            && (!runtime.config.foreground_only || game_foreground())
    );
    if show == runtime.shown { return Ok(()); }
    let win = window(app)?;
    if show { win.show().map_err(err)?; } else { win.hide().map_err(err)?; }
    runtime.shown = show;
    Ok(())
}

pub fn hide(app: &AppHandle, shared: &Arc<Shared>) {
    let mut state = lock(&shared.overlay);
    state.clear();
    let _ = sync(app, &mut state);
}

#[tauri::command]
pub async fn get_overlay_settings(shared: State<'_, Arc<Shared>>) -> Result<Settings, String> {
    Ok(lock(&shared.overlay).settings())
}
#[tauri::command]
pub async fn get_overlay_frame(shared: State<'_, Arc<Shared>>) -> Result<Frame, String> {
    Ok(lock(&shared.overlay).frame())
}
#[tauri::command]
pub async fn set_overlay_settings(
    app: AppHandle,
    shared: State<'_, Arc<Shared>>,
    mut config: Config,
) -> Result<Settings, String> {
    config.normalize();
    let mut state = lock(&shared.overlay);
    let resize = config.scale != state.config.scale;
    config.x = state.config.x;
    config.y = state.config.y;
    config.width = if resize { None } else { state.config.width };
    config.height = if resize { None } else { state.config.height };
    if resize { apply_geometry(&window(&app)?, &config)?; }
    let json = serde_json::to_string(&config).map_err(err)?;
    lock(&shared.store).set_setting("overlay_config", &json).map_err(err)?;
    state.config = config;
    sync(&app, &mut state)?;
    Ok(state.settings())
}
#[tauri::command]
pub async fn edit_overlay(
    app: AppHandle,
    shared: State<'_, Arc<Shared>>,
    editing: bool,
) -> Result<Settings, String> {
    let mut state = lock(&shared.overlay);
    if !state.ready { return Err("Overlay 尚未初始化".into()); }
    let win = window(&app)?;
    if editing {
        apply_geometry(&win, &state.config)?;
        win.set_focusable(true).map_err(err)?;
        win.set_ignore_cursor_events(false).map_err(err)?;
        state.editing = true;
        sync(&app, &mut state)?;
        win.set_focus().map_err(err)?;
    } else if state.editing {
        let pos = win.outer_position().map_err(err)?;
        let size = win.inner_size().map_err(err)?;
        let scale = win.scale_factor().map_err(err)?;
        let mut config = state.config.clone();
        config.x = Some(pos.x);
        config.y = Some(pos.y);
        config.width = Some(size.width as f64 / scale);
        config.height = Some(size.height as f64 / scale);
        config.normalize();
        let json = serde_json::to_string(&config).map_err(err)?;
        lock(&shared.store).set_setting("overlay_config", &json).map_err(err)?;
        state.config = config;
        win.set_ignore_cursor_events(true).map_err(err)?;
        win.set_focusable(false).map_err(err)?;
        state.editing = false;
        sync(&app, &mut state)?;
    }
    Ok(state.settings())
}
#[tauri::command]
pub async fn reset_overlay(app: AppHandle, shared: State<'_, Arc<Shared>>) -> Result<Settings, String> {
    let mut state = lock(&shared.overlay);
    let mut config = Config::default();
    config.enabled = state.config.enabled;
    apply_geometry(&window(&app)?, &config)?;
    let json = serde_json::to_string(&config).map_err(err)?;
    lock(&shared.store).set_setting("overlay_config", &json).map_err(err)?;
    state.config = config;
    sync(&app, &mut state)?;
    Ok(state.settings())
}
#[tauri::command]
pub async fn drag_overlay(
    window: tauri::Window,
    shared: State<'_, Arc<Shared>>,
    resize: bool,
) -> Result<(), String> {
    if window.label() != "overlay" || !lock(&shared.overlay).editing {
        return Err("Overlay 未处于调整模式".into());
    }
    if resize {
        window.start_resize_dragging(tauri_runtime::ResizeDirection::SouthEast).map_err(err)
    } else {
        window.start_dragging().map_err(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clamp_invalid_config() {
        let mut config = Config { opacity: 0, scale: 255, width: Some(f64::NAN), ..Default::default() };
        config.normalize();
        assert_eq!(config.opacity, 20);
        assert_eq!(config.scale, 200);
        assert_eq!(config.width, None);
    }
    #[test]
    fn end_time_is_frozen() {
        let mut runtime = Runtime::new(Config::default());
        runtime.live("run".to_owned(), 14.0);
        runtime.finished(16.25);
        assert_eq!(runtime.frame().elapsed_seconds, Some(16.25));
        assert!(runtime.frame().finished);
    }
}
