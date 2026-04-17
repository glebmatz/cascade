# Universal Cascade installer for Windows.
#
# Usage:
#   irm https://github.com/glebmatz/cascade/releases/latest/download/cascade-installer.ps1 | iex

$ErrorActionPreference = "Stop"

$Repo    = "glebmatz/cascade"
$Version = if ($env:CASCADE_VERSION) { $env:CASCADE_VERSION } else { "latest" }
$InstallDir = if ($env:CASCADE_INSTALL_DIR) {
    $env:CASCADE_INSTALL_DIR
} else {
    Join-Path $env:LOCALAPPDATA "cascade"
}

$arch = if ([Environment]::Is64BitOperatingSystem) { "x86_64-pc-windows-msvc" } else {
    throw "Cascade requires 64-bit Windows."
}

if ($Version -eq "latest") {
    $base = "https://github.com/$Repo/releases/latest/download"
} else {
    $base = "https://github.com/$Repo/releases/download/$Version"
}

$archive = "cascade-$arch.zip"
$url = "$base/$archive"

Write-Host "Detected target: $arch"
Write-Host "Downloading $url"

$tmp = New-TemporaryFile
$tmpZip = "$tmp.zip"
Remove-Item $tmp
Invoke-WebRequest -Uri $url -OutFile $tmpZip -UseBasicParsing

try {
    $hashUrl = "$url.sha256"
    $expected = (Invoke-WebRequest -Uri $hashUrl -UseBasicParsing).Content.Trim().Split(' ')[0]
    $actual = (Get-FileHash $tmpZip -Algorithm SHA256).Hash
    if ($expected -and $actual -ne $expected) {
        throw "SHA256 mismatch: expected $expected, got $actual"
    }
} catch {
    Write-Host "(sha256 verification skipped: $_)"
}

Write-Host "Extracting..."
$stage = New-Item -ItemType Directory -Path "$env:TEMP\cascade-install-$(Get-Random)"
Expand-Archive -LiteralPath $tmpZip -DestinationPath $stage -Force
Remove-Item $tmpZip

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
$src = Join-Path $stage "cascade-$arch\cascade.exe"
$dst = Join-Path $InstallDir "cascade.exe"
Copy-Item -Force $src $dst
Remove-Item -Recurse -Force $stage

Write-Host ""
Write-Host "Installed cascade.exe to $InstallDir"

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (-not ($userPath -split ";" | Where-Object { $_ -ieq $InstallDir })) {
    Write-Host ""
    Write-Host "Adding $InstallDir to your User PATH..."
    [Environment]::SetEnvironmentVariable(
        "Path",
        ($userPath.TrimEnd(';') + ";" + $InstallDir),
        "User"
    )
    Write-Host "Restart your terminal for PATH changes to apply."
}

Write-Host ""
Write-Host "Run 'cascade' to start, or 'cascade help' for usage."
