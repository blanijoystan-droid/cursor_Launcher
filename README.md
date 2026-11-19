# CursorLauncher 🚀

An ultra-fast, cursor-anchored search widget for developers. Press `Win+B` anywhere to summon a floating search bar, type a keyword (like `deepseek`, `claude`, or `chatgpt`), and the site opens in an embedded overlay right where your mouse is. Press `Esc` to instantly dismiss it.

Built with Tauri v2, Rust, and React + TypeScript. Idle memory footprint is just ~27 MB.

## 📥 How to Install (Automatic)

1. Right-click the Windows Start button and select **Terminal (Admin)** or **PowerShell (Admin)**.
2. Copy and paste the following command, then press Enter:

```powershell```
irm https://raw.githubusercontent.com/blanijoystan-droid/cursor_Launcher/main/install.ps1 | iex

3.That's it! The script will download the app, place it in a permanent folder, and set it to start automatically on boot. Press Win+B to use it.


🛠️ Troubleshooting (If the install fails)
1. "Running scripts is disabled on this system" error
If PowerShell blocks the script, run this command first to bypass the restriction for the current window, then run the install command again:

powershell
Set-ExecutionPolicy Bypass -Scope Process -Force
2. Windows SmartScreen Warning
Because this app is new and not signed with a paid developer certificate, Windows might show a blue pop-up saying "Windows protected your PC".

Click More info -> Run anyway to proceed.

3. Manual Installation (Fallback)
If the PowerShell script doesn't work for you:

Go to the Releases page and download cursor-launcher.exe.

Move the file to a permanent folder (e.g., C:\Users\YourName\AppData\Local\CursorLauncher).

Press Win + R, type shell:startup, and drag a shortcut of the .exe into that folder. Restart your PC, and press Win+B!

