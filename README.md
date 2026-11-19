# CursorLauncher 🚀

**CursorLauncher** is an ultra-fast, lightweight, cross-platform desktop utility built with **Tauri v2**, **Rust**, and **React + TypeScript**.

It anchors a floating, frameless search box directly at your cursor coordinates upon pressing **Win+B** (Super+B), allowing you to route to websites (DeepSeek, Claude, Stack Overflow, etc.) in an embedded floating overlay window, and dismiss instantly with **Esc**.

---

## ⚡ Core Features

- **Global Hotkey (Win+B)**: Anchors a transient search bar directly at the cursor coordinates across any monitor, respecting multi-monitor configurations, negative coordinates, and per-monitor DPI scaling.
- **Windows System-Tray Override**: Overcomes the default Windows `Win+B` system-tray conflict using a **dual-registration approach**:
  1. High-level registration via `tauri-plugin-global-shortcut`.
  2. Low-level Win32 keyboard hook (`WH_KEYBOARD_LL`) that intercepts and consumes the `Win+B` event before Windows Explorer processes it.
- **DWM Zero-Latency Animations**: Uses `DWMWA_TRANSITIONS_FORCEDISABLED` via `windows-sys` to eliminate Windows DWM window show/fade animations, opening the search bar instantly like Raycast.
- **True Native Embedded Webview (No iframes)**: Navigates the overlay `WebviewWindow` directly to the target URL (`overlay_w.navigate(url)`). Completely avoids `X-Frame-Options` and CSP `frame-ancestors` restrictions.
- **Fuzzy Keyword Routing**:
  - `deepseek` / `ds` ➔ `https://chat.deepseek.com`
  - `claude` / `cl` ➔ `https://claude.ai`
  - `so` / `stackoverflow` ➔ `https://stackoverflow.com`
  - `chatgpt` / `gpt` ➔ `https://chatgpt.com`
  - `github` / `gh` ➔ `https://github.com`
  - `google` / `g` ➔ `https://www.google.com`
  - Direct URL detection (`https://...` or `domain.com`)
  - Web search query fallback (`google.com/search?q=...`)
  - Interactive Custom Keyword Creator with `localStorage` persistence.
- **Transient Focus & Clean Dismissal (Esc)**:
  - Captures the active application's HWND (`GetForegroundWindow`) before opening.
  - Automatically restores focus to the previously active application upon pressing `Esc`.
  - Dismissal via `Esc` only; overlay remains visible on blur so you can easily copy and reference text.
- **Ultra-Low Memory Footprint**:
  - Idle Working Set: **~27 MB** (Well below the 80 MB limit)
  - Standalone Binary Size: **4.39 MB**

---

## 🛠️ Project Architecture

```
cursor_Launcher/
├── src-tauri/
│   ├── src/
│   │   ├── lib.rs          # Core Rust engine: WH_KEYBOARD_LL hook, multi-monitor clamping,
│   │   │                   # focus management, DWM transition disable, window routing
│   │   └── main.rs         # Entry point (windows_subsystem = "windows")
│   ├── Cargo.toml          # Tauri v2, tauri-plugin-global-shortcut, windows-sys, serde, url
│   └── tauri.conf.json     # Multi-window config: 'main', 'search' (600x80), 'overlay' (800x600)
├── src/
│   ├── components/
│   │   ├── SearchWindow.tsx  # Frameless search input with frosted glass & fuzzy routing
│   │   ├── OverlayWindow.tsx # Native webview overlay container
│   │   └── MainWindow.tsx    # Hidden background daemon
│   ├── utils/
│   │   └── routing.ts        # Keyword dictionary, fuzzy matcher, custom mapping storage
│   ├── styles/
│   │   └── search.css        # Raycast-inspired dark frosted glass styling
│   └── App.tsx               # Window router based on ?window= parameter
├── cursor-launcher.exe     # Pre-compiled standalone release executable (4.39 MB)
└── package.json            # React, TypeScript, Vite, Tauri v2 CLI
```

---

## 🚀 Running the App

### Option A: Run the Pre-compiled Executable Directly
Simply launch:
```powershell
.\cursor-launcher.exe
```
Then press **Win+B** anywhere on your desktop!

### Option B: Build from Source
1. Ensure Rust (`stable-x86_64-pc-windows-msvc`) and Node.js are installed.
2. Build the frontend:
   ```powershell
   npm install
   npm run build
   ```
3. Run or compile the Tauri binary:
   ```powershell
   cd src-tauri
   cargo build --release
   ```
