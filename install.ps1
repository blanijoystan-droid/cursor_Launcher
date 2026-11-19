# CursorLauncher One-Liner Installer
$repo = "blanijoystan-droid/cursor_Launcher" 
$exeName = "cursor-launcher.exe"
$installDir = "$env:LOCALAPPDATA\CursorLauncher"

Write-Host "Setting up CursorLauncher..." -ForegroundColor Cyan

# 1. Create a permanent installation directory
if (-not (Test-Path $installDir)) {
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
}

# 2. Find the latest release download URL from GitHub
$latestRelease = Invoke-RestMethod -Uri "https://api.github.com/repos/$repo/releases/latest"
$asset = $latestRelease.assets | Where-Object { $_.name -eq $exeName }

if (-not $asset) {
    Write-Error "Could not find $exeName in the latest release. Make sure you uploaded it to GitHub Releases!"
    exit 1
}

$downloadUrl = $asset.browser_download_url

# 3. Download the executable
Write-Host "Downloading CursorLauncher from GitHub..." -ForegroundColor Yellow
Invoke-WebRequest -Uri $downloadUrl -OutFile "$installDir\$exeName"

# 4. Add to Startup Registry (so it runs every time)
$registryPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
$registryName = "CursorLauncher"
Set-ItemProperty -Path $registryPath -Name $registryName -Value "`"$installDir\$exeName`""

Write-Host "✅ Installation complete!" -ForegroundColor Green
Write-Host "CursorLauncher will now start automatically when you turn on your PC."
Write-Host "Press Win+B to open the search bar."