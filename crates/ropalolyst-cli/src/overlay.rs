use chrono::{DateTime, Utc};
use std::time::{Duration, Instant};

#[cfg(windows)]
mod platform {
    use super::*;
    use std::{
        ptr::{null, null_mut},
        sync::{
            Arc, Mutex, OnceLock,
            atomic::{AtomicBool, AtomicIsize, Ordering},
        },
        thread::{self, JoinHandle},
    };
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{
            BeginPaint, CreateFontW, CreateSolidBrush, DEFAULT_CHARSET, DEFAULT_PITCH,
            DeleteObject, DrawTextW, EndPaint, FF_DONTCARE, FillRect, FW_BOLD,
            OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, SelectObject,
            SetBkMode, SetTextColor, TRANSPARENT,
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DispatchMessageW, InvalidateRect,
            PeekMessageW, PostMessageW, PostQuitMessage, RegisterClassW, SetLayeredWindowAttributes,
            SetWindowPos, ShowWindow, TranslateMessage, WNDCLASSW, MSG, PAINTSTRUCT,
            CW_USEDEFAULT, HWND_TOPMOST, LWA_ALPHA, MA_NOACTIVATE, PM_REMOVE, SW_HIDE,
            SW_SHOWNOACTIVATE, SWP_NOACTIVATE, WM_CLOSE, WM_DESTROY, WM_MOUSEACTIVATE,
            WM_NCHITTEST, WM_PAINT, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
            WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP, HTTRANSPARENT,
        },
    };

    const WINDOW_WIDTH: i32 = 360;
    const WINDOW_HEIGHT: i32 = 92;
    static DATA: OnceLock<Mutex<OverlayData>> = OnceLock::new();

    #[derive(Default)]
    struct OverlayData {
        started: Option<Instant>,
        frozen: Option<Duration>,
        visible: bool,
    }

    fn data() -> &'static Mutex<OverlayData> {
        DATA.get_or_init(|| Mutex::new(OverlayData::default()))
    }

    pub struct OverlayController {
        stop: Arc<AtomicBool>,
        hwnd: Arc<AtomicIsize>,
        thread: Option<JoinHandle<()>>,
    }

    impl OverlayController {
        pub fn new() -> Self {
            let stop = Arc::new(AtomicBool::new(false));
            let hwnd = Arc::new(AtomicIsize::new(0));
            let thread_stop = Arc::clone(&stop);
            let thread_hwnd = Arc::clone(&hwnd);
            let thread = thread::Builder::new()
                .name("ropalolyst-overlay".into())
                .spawn(move || run_window(thread_stop, thread_hwnd))
                .ok();
            Self { stop, hwnd, thread }
        }

        pub fn start(&self, mission_started_at: DateTime<Utc>) {
            let elapsed = Utc::now()
                .signed_duration_since(mission_started_at)
                .to_std()
                .unwrap_or(Duration::ZERO);
            if let Ok(mut state) = data().lock() {
                state.started = Instant::now().checked_sub(elapsed).or_else(|| Some(Instant::now()));
                state.frozen = None;
                state.visible = true;
            }
        }

        pub fn finish(&self, elapsed: Duration) {
            if let Ok(mut state) = data().lock() {
                state.frozen = Some(elapsed);
                state.visible = true;
            }
        }

        pub fn hide(&self) {
            if let Ok(mut state) = data().lock() {
                state.visible = false;
                state.started = None;
                state.frozen = None;
            }
        }
    }

    impl Drop for OverlayController {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            let hwnd = self.hwnd.load(Ordering::Relaxed) as HWND;
            if !hwnd.is_null() {
                unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0); }
            }
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    fn run_window(stop: Arc<AtomicBool>, hwnd_slot: Arc<AtomicIsize>) {
        let class_name: Vec<u16> = "RopalolystAnalyticsOverlay".encode_utf16().chain(Some(0)).collect();
        let title: Vec<u16> = "Ropalolyst Analytics Timer".encode_utf16().chain(Some(0)).collect();
        unsafe {
            let instance = GetModuleHandleW(null());
            let class = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(window_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: instance,
                hIcon: null_mut(),
                hCursor: null_mut(),
                hbrBackground: null_mut(),
                lpszMenuName: null(),
                lpszClassName: class_name.as_ptr(),
            };
            if RegisterClassW(&class) == 0 {
                return;
            }

            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_POPUP,
                CW_USEDEFAULT,
                16,
                WINDOW_WIDTH,
                WINDOW_HEIGHT,
                null_mut(),
                null_mut(),
                instance,
                null(),
            );
            if hwnd.is_null() {
                return;
            }
            hwnd_slot.store(hwnd as isize, Ordering::Relaxed);
            SetLayeredWindowAttributes(hwnd, 0, 232, LWA_ALPHA);
            let screen_width = windows_sys::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
                windows_sys::Win32::UI::WindowsAndMessaging::SM_CXSCREEN,
            );
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                (screen_width - WINDOW_WIDTH) / 2,
                18,
                WINDOW_WIDTH,
                WINDOW_HEIGHT,
                SWP_NOACTIVATE,
            );
            ShowWindow(hwnd, SW_HIDE);

            let mut was_visible = false;
            let mut message: MSG = std::mem::zeroed();
            while !stop.load(Ordering::Relaxed) {
                while PeekMessageW(&mut message, null_mut(), 0, 0, PM_REMOVE) != 0 {
                    if message.message == windows_sys::Win32::UI::WindowsAndMessaging::WM_QUIT {
                        stop.store(true, Ordering::Relaxed);
                        break;
                    }
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }

                let visible = data().lock().map(|state| state.visible).unwrap_or(false);
                if visible != was_visible {
                    ShowWindow(hwnd, if visible { SW_SHOWNOACTIVATE } else { SW_HIDE });
                    was_visible = visible;
                }
                if visible {
                    InvalidateRect(hwnd, null(), 0);
                }
                thread::sleep(Duration::from_millis(16));
            }
            hwnd_slot.store(0, Ordering::Relaxed);
            if !hwnd.is_null() {
                windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
            }
        }
    }

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            WM_PAINT => {
                let mut paint: PAINTSTRUCT = unsafe { std::mem::zeroed() };
                let hdc = unsafe { BeginPaint(hwnd, &mut paint) };
                let rect = RECT { left: 0, top: 0, right: WINDOW_WIDTH, bottom: WINDOW_HEIGHT };
                let brush = unsafe { CreateSolidBrush(0x00221A18) };
                unsafe { FillRect(hdc, &rect, brush); DeleteObject(brush); }
                unsafe { SetBkMode(hdc, TRANSPARENT); }

                let (visible, elapsed) = data().lock().map(|state| {
                    let elapsed = state.frozen.or_else(|| state.started.map(|start| start.elapsed())).unwrap_or(Duration::ZERO);
                    (state.visible, elapsed)
                }).unwrap_or((false, Duration::ZERO));
                if visible {
                    let label: Vec<u16> = "ROPALOLYST ANALYTICS".encode_utf16().chain(Some(0)).collect();
                    let timer: Vec<u16> = format_duration(elapsed).encode_utf16().chain(Some(0)).collect();

                    let face: Vec<u16> = "Segoe UI".encode_utf16().chain(Some(0)).collect();
                    let small_font = unsafe { CreateFontW(14, 0, 0, 0, FW_BOLD, 0, 0, 0, DEFAULT_CHARSET as u32, OUT_DEFAULT_PRECIS as u32, CLIP_DEFAULT_PRECIS as u32, CLEARTYPE_QUALITY as u32, DEFAULT_PITCH | FF_DONTCARE, face.as_ptr()) };
                    let old_font = unsafe { SelectObject(hdc, small_font) };
                    unsafe { SetTextColor(hdc, 0x00B8D8D0); }
                    let mut label_rect = RECT { left: 0, top: 8, right: WINDOW_WIDTH, bottom: 30 };
                    unsafe { DrawTextW(hdc, label.as_ptr(), -1, &mut label_rect, 0x00000001 | 0x00000020 | 0x00000800); }

                    let large_font = unsafe { CreateFontW(42, 0, 0, 0, FW_BOLD, 0, 0, 0, DEFAULT_CHARSET as u32, OUT_DEFAULT_PRECIS as u32, CLIP_DEFAULT_PRECIS as u32, CLEARTYPE_QUALITY as u32, DEFAULT_PITCH | FF_DONTCARE, face.as_ptr()) };
                    unsafe { SelectObject(hdc, large_font); SetTextColor(hdc, 0x00FFFFFF); }
                    let mut timer_rect = RECT { left: 0, top: 25, right: WINDOW_WIDTH, bottom: 86 };
                    unsafe { DrawTextW(hdc, timer.as_ptr(), -1, &mut timer_rect, 0x00000001 | 0x00000004 | 0x00000800); }
                    unsafe { SelectObject(hdc, old_font); DeleteObject(small_font); DeleteObject(large_font); }
                }
                unsafe { EndPaint(hwnd, &paint); }
                0
            }
            WM_NCHITTEST => HTTRANSPARENT as LRESULT,
            WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
            WM_DESTROY => {
                unsafe { PostQuitMessage(0); }
                0
            }
            _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
        }
    }

    fn format_duration(duration: Duration) -> String {
        let total_ms = duration.as_millis();
        let minutes = total_ms / 60_000;
        let seconds = (total_ms / 1_000) % 60;
        let millis = total_ms % 1_000;
        format!("{minutes:02}:{seconds:02}.{millis:03}")
    }
}

#[cfg(not(windows))]
mod platform {
    use super::*;

    pub struct OverlayController;

    impl OverlayController {
        pub fn new() -> Self { Self }
        pub fn start(&self, _mission_started_at: DateTime<Utc>) {}
        pub fn finish(&self, _elapsed: Duration) {}
        pub fn hide(&self) {}
    }
}

pub use platform::OverlayController;
