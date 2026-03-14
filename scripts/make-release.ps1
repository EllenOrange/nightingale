$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location "$ScriptDir\.."

$Target = "x86_64-pc-windows-msvc"
Write-Host "==> Platform: $Target"

# ─── Vendor binaries ─────────────────────────────────────────────────

if (-not (Test-Path "vendor-bin")) {
    New-Item -ItemType Directory -Path "vendor-bin" | Out-Null
}

# ffmpeg
if (-not (Test-Path "vendor-bin\ffmpeg.exe")) {
    Write-Host "Downloading ffmpeg..."
    $ffmpegZip = "$env:TEMP\ffmpeg.zip"
    Invoke-WebRequest -Uri "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip" -OutFile $ffmpegZip
    $extractDir = "$env:TEMP\ffmpeg_extract"
    Expand-Archive -Path $ffmpegZip -DestinationPath $extractDir -Force
    $ffmpegExe = Get-ChildItem -Path $extractDir -Recurse -Filter "ffmpeg.exe" | Select-Object -First 1
    Copy-Item $ffmpegExe.FullName "vendor-bin\ffmpeg.exe"
    Remove-Item $ffmpegZip -Force
    Remove-Item $extractDir -Recurse -Force
    Write-Host "ffmpeg downloaded"
} else {
    Write-Host "ffmpeg already present"
}

# uv
if (-not (Test-Path "vendor-bin\uv.exe")) {
    Write-Host "Downloading uv..."
    $uvZip = "$env:TEMP\uv.zip"
    Invoke-WebRequest -Uri "https://github.com/astral-sh/uv/releases/latest/download/uv-x86_64-pc-windows-msvc.zip" -OutFile $uvZip
    $extractDir = "$env:TEMP\uv_extract"
    Expand-Archive -Path $uvZip -DestinationPath $extractDir -Force
    $uvExe = Get-ChildItem -Path $extractDir -Recurse -Filter "uv.exe" | Select-Object -First 1
    Copy-Item $uvExe.FullName "vendor-bin\uv.exe"
    Remove-Item $uvZip -Force
    Remove-Item $extractDir -Recurse -Force
    Write-Host "uv downloaded"
} else {
    Write-Host "uv already present"
}

Write-Host "vendor-bin/ ready"
Get-ChildItem "vendor-bin" | Format-Table Name, Length -AutoSize

# ─── Build ───────────────────────────────────────────────────────────

Write-Host "==> Building release binary..."
if (Test-Path ".env") {
    Get-Content ".env" | ForEach-Object {
        if ($_ -match '^\s*([^#][^=]+)=(.*)$') {
            [Environment]::SetEnvironmentVariable($Matches[1].Trim(), $Matches[2].Trim(), "Process")
        }
    }
}
cargo build --release --target $Target
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# ─── Package ─────────────────────────────────────────────────────────

$Binary = "target\$Target\release\nightingale.exe"
$Archive = "nightingale-$Target.zip"

Write-Host "==> Packaging $Archive..."
Compress-Archive -Path $Binary -DestinationPath $Archive -Force

$BinarySize = (Get-Item $Binary).Length / 1MB
$ArchiveSize = (Get-Item $Archive).Length / 1MB

Write-Host ""
Write-Host "Done!"
Write-Host ("  Binary:  $Binary ({0:N1} MB)" -f $BinarySize)
Write-Host ("  Archive: $Archive ({0:N1} MB)" -f $ArchiveSize)
