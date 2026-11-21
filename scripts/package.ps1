# Package the already-built installer with separate, optional offline languages.
param([string]$Installer)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
if (-not $Installer) { $Installer = Join-Path $root 'target\release\plugin-launcher.exe' }
if (-not (Test-Path -LiteralPath $Installer -PathType Leaf)) { throw 'Build the installer first: scripts\build-all.ps1' }
$out = Join-Path $root 'dist\WinRust'
$locales = Join-Path $out 'locales'
New-Item -ItemType Directory -Force -Path $locales | Out-Null
Copy-Item -LiteralPath $Installer -Destination (Join-Path $out 'WinRust.exe') -Force
foreach ($id in @('pt-BR', 'ja', 'ru')) {
    & python (Join-Path $PSScriptRoot 'build-language-pack.py') (Join-Path $root "locales\$id.json") (Join-Path $locales "$id.wrlang")
    if ($LASTEXITCODE -ne 0) { throw "Failed to build language pack $id" }
}
$zip = Join-Path $root 'dist\WinRust-with-languages.zip'
Compress-Archive -Path (Join-Path $out '*') -DestinationPath $zip -Force
Write-Host "Installer and language packs: $out"
Write-Host "Offline package: $zip"
