$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location "$ScriptDir\.."

$Target = "x86_64-pc-windows-msvc"
Write-Host "==> Platform: $Target"

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
