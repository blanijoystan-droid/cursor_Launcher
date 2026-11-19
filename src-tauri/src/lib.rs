use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter, Listener, Manager, WebviewWindow};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

const CHROME_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

const ESC_INJECTION_SCRIPT: &str = r#"
(function() {
    try {
        Object.defineProperty(navigator, 'webdriver', {
            get: () => false,
        });
    } catch(e) {}

    // 1. Intercept Esc key in capture phase
    function handleEsc(e) {
        if (e.key === 'Escape' || e.keyCode === 27) {
            try {
                if (window.__TAURI__ && window.__TAURI__.event && window.__TAURI__.event.emit) {
                    window.__TAURI__.event.emit('close-overlay-window');
                }
                if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {
                    window.__TAURI_INTERNALS__.invoke('dismiss');
                }
                if (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke) {
                    window.__TAURI__.core.invoke('dismiss');
                }
            } catch (err) {
                console.error('[CursorLauncher] Failed to emit close-overlay-window:', err);
            }
        }
    }
    window.addEventListener('keydown', handleEsc, true);
    document.addEventListener('keydown', handleEsc, true);

    // 2. Intercept window.open calls to navigate in the same WebView
    try {
        var originalWindowOpen = window.open;
        window.open = function(url, target, features) {
            if (url) {
                window.location.href = url;
                return window;
            }
            return originalWindowOpen ? originalWindowOpen.apply(window, arguments) : null;
        };
    } catch(e) {}

    // 3. Intercept clicks on links with target="_blank" to navigate in the same WebView
    document.addEventListener('click', function(e) {
        try {
            var target = e.target;
            while (target && target.tagName !== 'A') {
                target = target.parentElement;
            }
            if (target && target.href) {
                if (target.target === '_blank' || target.getAttribute('target') === '_blank') {
                    e.preventDefault();
                    e.stopPropagation();
                    window.location.href = target.href;
                }
            }
        } catch(e) {}
    }, true);
})();
"#;

static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();
static PREV_FOREGROUND_HWND: AtomicIsize = AtomicIsize::new(0);
static IS_SEARCH_VISIBLE: AtomicBool = AtomicBool::new(false);
static IS_OVERLAY_VISIBLE: AtomicBool = AtomicBool::new(false);
static IS_ANY_WINDOW_VISIBLE: AtomicBool = AtomicBool::new(false);
static LAST_TRIGGER_TIME: AtomicU64 = AtomicU64::new(0);
static LAST_CURSOR_POS: AtomicU64 = AtomicU64::new(0);
static LAST_SEARCH_RECT: AtomicU64 = AtomicU64::new(0);

#[cfg(windows)]
static mut HOOK_HANDLE: windows_sys::Win32::UI::WindowsAndMessaging::HHOOK = std::ptr::null_mut();

/// Helper to get current timestamp in milliseconds
fn current_time_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Append a timestamped message to the boot log file at %LOCALAPPDATA%\CursorLauncher\boot.log
fn log_boot(msg: &str) {
    #[cfg(windows)]
    {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let log_dir = std::path::PathBuf::from(local_app_data).join("CursorLauncher");
            let _ = std::fs::create_dir_all(&log_dir);
            let log_path = log_dir.join("boot.log");
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
            {
                use std::io::Write;
                let _ = writeln!(file, "[{}] {}", current_time_millis(), msg);
            }
        }
    }
}

/// Explicitly update HKCU\Software\Microsoft\Windows\CurrentVersion\Run with quoted path
#[cfg(windows)]
fn set_autostart_registry_quoted(app_name: &str, exe_path: &std::path::Path) -> std::io::Result<()> {
    let quoted_path = format!("\"{}\"", exe_path.to_string_lossy());
    let hkcu = windows_registry::CURRENT_USER;
    let key = hkcu
        .create(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{:?}", e)))?;
    key.set_string(app_name, &quoted_path)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{:?}", e)))?;
    Ok(())
}

/// Disable DWM transition / fade animations on Windows
pub fn disable_window_animations(window: &WebviewWindow) {
    #[cfg(windows)]
    if let Ok(hwnd) = window.hwnd() {
        unsafe {
            let disable: windows_sys::Win32::Foundation::BOOL = 1;
            windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
                hwnd.0 as windows_sys::Win32::Foundation::HWND,
                windows_sys::Win32::Graphics::Dwm::DWMWA_TRANSITIONS_FORCEDISABLED as u32,
                &disable as *const _ as _,
                std::mem::size_of::<windows_sys::Win32::Foundation::BOOL>() as u32,
            );
        }
    }
}

/// Capture the current foreground window before showing the launcher
fn capture_previous_foreground(app: &AppHandle) {
    #[cfg(windows)]
    unsafe {
        let current_fg = windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
        if !current_fg.is_null() {
            let search_hwnd = app
                .get_webview_window("search")
                .and_then(|w| w.hwnd().ok())
                .map(|h| h.0 as isize)
                .unwrap_or(0);
            let overlay_hwnd = app
                .get_webview_window("overlay")
                .and_then(|w| w.hwnd().ok())
                .map(|h| h.0 as isize)
                .unwrap_or(0);

            let current_isize = current_fg as isize;
            if current_isize != search_hwnd && current_isize != overlay_hwnd {
                PREV_FOREGROUND_HWND.store(current_isize, Ordering::SeqCst);
            }
        }
    }
}

/// Restore focus to previously active application
fn restore_previous_foreground() {
    #[cfg(windows)]
    {
        let prev = PREV_FOREGROUND_HWND.swap(0, Ordering::SeqCst);
        if prev != 0 {
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow(
                    prev as windows_sys::Win32::Foundation::HWND,
                );
            }
        }
    }
}

/// Core function: displays the search box centered at the cursor coordinates,
/// clamped properly to the containing monitor boundaries.
pub fn show_search_window(app: &AppHandle) {
    let now = current_time_millis();
    let last = LAST_TRIGGER_TIME.load(Ordering::SeqCst);
    if now.saturating_sub(last) < 250 {
        // Debounce dual-registration / rapid triggers
        return;
    }
    LAST_TRIGGER_TIME.store(now, Ordering::SeqCst);

    capture_previous_foreground(app);

    let search_window = match app.get_webview_window("search") {
        Some(w) => w,
        None => {
            eprintln!("[CursorLauncher] Search window not found");
            return;
        }
    };

    // If overlay window was open, hide it and reset to blank page
    if let Some(overlay_w) = app.get_webview_window("overlay") {
        let _ = overlay_w.hide();
        if let Ok(blank_url) = "about:blank".parse() {
            let _ = overlay_w.navigate(blank_url);
        }
        IS_OVERLAY_VISIBLE.store(false, Ordering::SeqCst);
    }

    // Query physical cursor coordinates
    let cursor_pos = match app.cursor_position() {
        Ok(pos) => pos,
        Err(_) => {
            #[cfg(windows)]
            {
                let mut pt = windows_sys::Win32::Foundation::POINT { x: 0, y: 0 };
                unsafe {
                    windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut pt);
                }
                tauri::PhysicalPosition {
                    x: pt.x as f64,
                    y: pt.y as f64,
                }
            }
            #[cfg(not(windows))]
            tauri::PhysicalPosition { x: 0.0, y: 0.0 }
        }
    };

    let cx = cursor_pos.x.round() as i32;
    let cy = cursor_pos.y.round() as i32;

    // Cache cursor position for subsequent overlay positioning
    let packed_cursor = ((cx as u64) << 32) | (cy as u32 as u64);
    LAST_CURSOR_POS.store(packed_cursor, Ordering::SeqCst);

    // Find monitor containing cursor
    let monitors = app.available_monitors().unwrap_or_default();
    let monitor = monitors
        .into_iter()
        .find(|m| {
            let pos = m.position();
            let size = m.size();
            cx >= pos.x
                && cx < pos.x + size.width as i32
                && cy >= pos.y
                && cy < pos.y + size.height as i32
        })
        .or_else(|| app.primary_monitor().ok().flatten());

    let (mon_x, mon_y, mon_w, mon_h, scale_factor) = if let Some(m) = monitor {
        (
            m.position().x,
            m.position().y,
            m.size().width as i32,
            m.size().height as i32,
            m.scale_factor(),
        )
    } else {
        (0, 0, 1920, 1080, 1.0)
    };

    // Logical dimensions: 600x70
    let search_w_px = (600.0 * scale_factor).round() as i32;
    let search_h_px = (70.0 * scale_factor).round() as i32;

    let target_x = cx - (search_w_px / 2);
    let target_y = cy - (search_h_px / 2);

    let margin = (12.0 * scale_factor).round() as i32;
    let min_x = mon_x + margin;
    let max_x = mon_x + mon_w - search_w_px - margin;
    let min_y = mon_y + margin;
    let max_y = mon_y + mon_h - search_h_px - margin;

    let clamped_x = if max_x >= min_x {
        target_x.clamp(min_x, max_x)
    } else {
        min_x
    };
    let clamped_y = if max_y >= min_y {
        target_y.clamp(min_y, max_y)
    } else {
        min_y
    };

    let packed_rect = ((clamped_x as u64) << 32) | (clamped_y as u32 as u64);
    LAST_SEARCH_RECT.store(packed_rect, Ordering::SeqCst);

    let _ = search_window.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
        x: clamped_x,
        y: clamped_y,
    }));
    let _ = search_window.show();
    let _ = search_window.set_focus();
    let _ = search_window.emit("reset_search", ());

    IS_SEARCH_VISIBLE.store(true, Ordering::SeqCst);
    IS_ANY_WINDOW_VISIBLE.store(true, Ordering::SeqCst);
}

/// Hide search and overlay windows, and restore focus to previous application
#[tauri::command]
fn dismiss(app: AppHandle) {
    if let Some(search_w) = app.get_webview_window("search") {
        let _ = search_w.hide();
    }
    if let Some(overlay_w) = app.get_webview_window("overlay") {
        let _ = overlay_w.hide();
        // Reset WebView to about:blank on dismiss to eliminate flash of old content
        if let Ok(blank_url) = "about:blank".parse() {
            let _ = overlay_w.navigate(blank_url);
        }
    }
    IS_SEARCH_VISIBLE.store(false, Ordering::SeqCst);
    IS_OVERLAY_VISIBLE.store(false, Ordering::SeqCst);
    IS_ANY_WINDOW_VISIBLE.store(false, Ordering::SeqCst);

    restore_previous_foreground();
}

/// Open overlay window with target URL anchored relative to cursor / search box
#[tauri::command]
fn launch_overlay(app: AppHandle, target_url: String) -> Result<(), String> {
    // 1. Hide search window
    if let Some(search_w) = app.get_webview_window("search") {
        let _ = search_w.hide();
        IS_SEARCH_VISIBLE.store(false, Ordering::SeqCst);
    }

    let overlay_w = app
        .get_webview_window("overlay")
        .ok_or_else(|| "Overlay window not found".to_string())?;

    let parsed_url: url::Url = target_url
        .parse()
        .map_err(|e| format!("Invalid URL: {}", e))?;

    // Retrieve last search coordinates and cursor coordinates
    let search_rect_val = LAST_SEARCH_RECT.load(Ordering::SeqCst);
    let search_x = (search_rect_val >> 32) as i32;
    let search_y = search_rect_val as u32 as i32;

    let cursor_pos_val = LAST_CURSOR_POS.load(Ordering::SeqCst);
    let cx = (cursor_pos_val >> 32) as i32;

    // Determine containing monitor
    let monitors = app.available_monitors().unwrap_or_default();
    let monitor = monitors
        .into_iter()
        .find(|m| {
            let pos = m.position();
            let size = m.size();
            search_x >= pos.x
                && search_x < pos.x + size.width as i32
                && search_y >= pos.y
                && search_y < pos.y + size.height as i32
        })
        .or_else(|| app.primary_monitor().ok().flatten());

    let (mon_x, mon_y, mon_w, mon_h, scale_factor) = if let Some(m) = monitor {
        (
            m.position().x,
            m.position().y,
            m.size().width as i32,
            m.size().height as i32,
            m.scale_factor(),
        )
    } else {
        (0, 0, 1920, 1080, 1.0)
    };

    let search_h_px = (70.0 * scale_factor).round() as i32;
    let overlay_w_px = (600.0 * scale_factor).round() as i32;
    let overlay_h_px = (750.0 * scale_factor).round() as i32;
    let margin = (12.0 * scale_factor).round() as i32;
    let gap = (8.0 * scale_factor).round() as i32;

    // Center horizontally around cursor
    let target_x = cx - (overlay_w_px / 2);
    let min_x = mon_x + margin;
    let max_x = mon_x + mon_w - overlay_w_px - margin;
    let clamped_x = if max_x >= min_x {
        target_x.clamp(min_x, max_x)
    } else {
        min_x
    };

    // Position vertically below search box, or above if close to bottom
    let below_y = search_y + search_h_px + gap;
    let max_y = mon_y + mon_h - overlay_h_px - margin;

    let clamped_y = if below_y <= max_y {
        below_y
    } else {
        let above_y = search_y - overlay_h_px - gap;
        if above_y >= mon_y + margin {
            above_y
        } else {
            below_y.clamp(mon_y + margin, max_y.max(mon_y + margin))
        }
    };

    let _ = overlay_w.set_size(tauri::Size::Logical(tauri::LogicalSize {
        width: 600.0,
        height: 750.0,
    }));

    let _ = overlay_w.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
        x: clamped_x,
        y: clamped_y,
    }));

    // Native Webview navigation (bypasses iframe restrictions entirely)
    // Navigate BEFORE showing so the new URL begins loading while invisible
    overlay_w
        .navigate(parsed_url)
        .map_err(|e| format!("Failed to navigate overlay: {}", e))?;

    // 50ms pause gives the WebView a moment to start fetching the new page while
    // the window is still invisible, completely masking any transition flash.
    std::thread::sleep(std::time::Duration::from_millis(50));

    let _ = overlay_w.show();
    let _ = overlay_w.set_focus();

    IS_OVERLAY_VISIBLE.store(true, Ordering::SeqCst);
    IS_ANY_WINDOW_VISIBLE.store(true, Ordering::SeqCst);

    Ok(())
}

/// Windows Low-Level Keyboard Hook procedure
#[cfg(windows)]
unsafe extern "system" fn ll_keyboard_proc(
    n_code: i32,
    w_param: windows_sys::Win32::Foundation::WPARAM,
    l_param: windows_sys::Win32::Foundation::LPARAM,
) -> windows_sys::Win32::Foundation::LRESULT {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_ESCAPE, VK_LWIN, VK_RWIN,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, KBDLLHOOKSTRUCT, WM_KEYDOWN, WM_SYSKEYDOWN,
    };

    if n_code >= 0 {
        let kb = unsafe { *(l_param as *const KBDLLHOOKSTRUCT) };
        let vk = kb.vkCode as i32;

        // Check for Win+B (0x42 is KeyB)
        if vk == 0x42 {
            let win_pressed = unsafe {
                (GetAsyncKeyState(VK_LWIN as i32) as u16 & 0x8000 != 0)
                    || (GetAsyncKeyState(VK_RWIN as i32) as u16 & 0x8000 != 0)
            };

            if win_pressed {
                if w_param as u32 == WM_KEYDOWN || w_param as u32 == WM_SYSKEYDOWN {
                    if let Some(app) = APP_HANDLE.get() {
                        let app_clone = app.clone();
                        std::thread::spawn(move || {
                            show_search_window(&app_clone);
                        });
                    }
                }
                // Return 1 to consume event: Windows Explorer system-tray focus is prevented!
                return 1;
            }
        }

        // Check for Escape (0x1B)
        if vk == VK_ESCAPE as i32 {
            if IS_ANY_WINDOW_VISIBLE.load(Ordering::SeqCst)
                || IS_SEARCH_VISIBLE.load(Ordering::SeqCst)
                || IS_OVERLAY_VISIBLE.load(Ordering::SeqCst)
            {
                if w_param as u32 == WM_KEYDOWN || w_param as u32 == WM_SYSKEYDOWN {
                    if let Some(app) = APP_HANDLE.get() {
                        let app_clone = app.clone();
                        std::thread::spawn(move || {
                            dismiss(app_clone);
                        });
                    }
                }
                // Consume Escape so background apps don't handle it
                return 1;
            }
        }
    }

    unsafe { CallNextHookEx(HOOK_HANDLE, n_code, w_param, l_param) }
}

/// Spawns a background thread that installs the WH_KEYBOARD_LL hook with a dedicated message pump.
#[cfg(windows)]
fn start_keyboard_hook_thread() {
    std::thread::spawn(|| {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage,
            UnhookWindowsHookEx, MSG, WH_KEYBOARD_LL,
        };

        unsafe {
            let hook = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(ll_keyboard_proc),
                std::ptr::null_mut(),
                0,
            );

            if hook.is_null() {
                eprintln!("[CursorLauncher Hook] Warning: Failed to install WH_KEYBOARD_LL hook");
                return;
            }

            HOOK_HANDLE = hook;
            let msg = "[Hook] WH_KEYBOARD_LL low-level hook installed successfully (Dual-Registration Active)";
            println!("{}", msg);
            log_boot(msg);

            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            UnhookWindowsHookEx(HOOK_HANDLE);
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        if shortcut.matches(Modifiers::SUPER, Code::KeyB) {
                            println!("[CursorLauncher GlobalShortcut] Super+B pressed via Tauri API");
                            show_search_window(app);
                        }
                    }
                })
                .build(),
        )
        .setup(|app| {
            let handle = app.handle().clone();
            let _ = APP_HANDLE.set(handle);

            // Phase 1: Block autostart for debug builds; register only for release builds
            if cfg!(debug_assertions) {
                let msg = "[Autostart] Debug build detected. Skipping autostart registration.";
                println!("{}", msg);
                log_boot(msg);
            } else {
                // Phase 2: Overwrite the registry entry unconditionally to point to the current release binary
                match std::env::current_exe() {
                    Ok(exe_path) => {
                        #[cfg(windows)]
                        match set_autostart_registry_quoted("CursorLauncher", &exe_path) {
                            Ok(_) => {
                                let msg = format!(
                                    "[Autostart] Successfully registered release binary in Windows Startup: \"{}\"",
                                    exe_path.display()
                                );
                                println!("{}", msg);
                                log_boot(&msg);
                            }
                            Err(e) => {
                                let msg = format!("[Autostart] Registry update failed: {}", e);
                                eprintln!("{}", msg);
                                log_boot(&msg);
                            }
                        }

                        // Also invoke the auto-launch manager to keep plugin state synchronized
                        let autostart_mgr = app.autolaunch();
                        let _ = autostart_mgr.enable();
                    }
                    Err(e) => {
                        let msg = format!("[Autostart] Could not determine current exe path: {:?}", e);
                        eprintln!("{}", msg);
                        log_boot(&msg);
                    }
                }
            }

            // Phase 3: Handle shortcut registration robustly with boot logging and AlreadyRegistered handling
            let app_handle_reg = app.handle().clone();
            std::thread::spawn(move || {
                let shortcut = Shortcut::new(Some(Modifiers::SUPER), Code::KeyB);

                log_boot("[Shortcut] Querying initial is_registered status for Super+B...");
                let initially_registered = app_handle_reg.global_shortcut().is_registered(shortcut);
                log_boot(&format!(
                    "[Shortcut] is_registered returned: {}",
                    initially_registered
                ));

                // If shortcut is already registered by another instance or previous state, unregister first
                if initially_registered {
                    log_boot("[Shortcut] Super+B was marked registered. Unregistering first to re-bind cleanly...");
                    let _ = app_handle_reg.global_shortcut().unregister(shortcut);
                }

                let mut registered = false;
                for attempt in 1..=5 {
                    if app_handle_reg.global_shortcut().is_registered(shortcut) {
                        let msg = format!(
                            "[Shortcut] Super+B confirmed registered on attempt {}",
                            attempt
                        );
                        println!("{}", msg);
                        log_boot(&msg);
                        registered = true;
                        break;
                    }

                    match app_handle_reg.global_shortcut().register(shortcut) {
                        Ok(_) => {
                            let msg = format!(
                                "[Shortcut] Super+B registered successfully via Tauri API on attempt {}.",
                                attempt
                            );
                            println!("{}", msg);
                            log_boot(&msg);
                            registered = true;
                            break;
                        }
                        Err(e) => {
                            let msg = format!(
                                "[Shortcut] Registration attempt {} failed: {:?}. Unregistering and retrying in 1.5s...",
                                attempt, e
                            );
                            eprintln!("{}", msg);
                            log_boot(&msg);
                            let _ = app_handle_reg.global_shortcut().unregister(shortcut);
                            std::thread::sleep(std::time::Duration::from_millis(1500));
                        }
                    }
                }

                if !registered {
                    let msg = "[Shortcut] Notice: Tauri API registration did not succeed. Low-level WH_KEYBOARD_LL hook is active and intercepts Win+B directly.";
                    eprintln!("{}", msg);
                    log_boot(msg);
                }
            });

            // Start low-level keyboard hook thread on Windows
            #[cfg(windows)]
            start_keyboard_hook_thread();

            // Disable DWM fade transitions on search window
            if let Some(search_w) = app.get_webview_window("search") {
                disable_window_animations(&search_w);
            }

            // Create the overlay WebviewWindow with a persistent data_directory
            // This ensures cookies, sessions, and Google sign-in persist on disk.
            let session_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join("webview_profile");
            let _ = std::fs::create_dir_all(&session_dir);

            #[cfg(windows)]
            unsafe {
                std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &session_dir);
            }

            let overlay_window = tauri::WebviewWindowBuilder::new(
                app,
                "overlay",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("CursorLauncher Overlay")
            .inner_size(600.0, 750.0)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(true)
            .visible(false)
            .shadow(true)
            .user_agent(CHROME_USER_AGENT)
            .initialization_script(ESC_INJECTION_SCRIPT)
            .data_directory(session_dir.clone())
            .on_new_window({
                let app_handle = app.handle().clone();
                move |url, _features| {
                    println!("[CursorLauncher] Intercepted new window request for: {}", url);
                    if let Some(overlay_w) = app_handle.get_webview_window("overlay") {
                        let _ = overlay_w.navigate(url);
                    }
                    tauri::webview::NewWindowResponse::Deny
                }
            })
            .build()?;

            // Listen for close-overlay-window event from the injected script in overlay WebView
            let app_handle_close = app.handle().clone();
            app.listen("close-overlay-window", move |_event| {
                println!("[CursorLauncher] Received close-overlay-window event");
                dismiss(app_handle_close.clone());
            });

            let app_handle_overlay_close = app.handle().clone();
            overlay_window.listen("close-overlay-window", move |_event| {
                println!("[CursorLauncher] Overlay window received close-overlay-window event");
                dismiss(app_handle_overlay_close.clone());
            });

            disable_window_animations(&overlay_window);

            println!(
                "[CursorLauncher] Overlay WebView created with persistent data_directory: {:?}",
                session_dir
            );

            println!("[CursorLauncher] Initialized successfully. Press Win+B to launch.");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![launch_overlay, dismiss])
        .run(tauri::generate_context!())
        .expect("error while running cursor launcher application");
}
