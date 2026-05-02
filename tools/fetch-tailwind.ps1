# Fetches the Tailwind CSS standalone CLI binary used by the Stroke
# build. The binary is gitignored; this script is the canonical way
# to (re)acquire it on a fresh checkout.
#
# Usage: pwsh tools/fetch-tailwind.ps1

$ErrorActionPreference = "Stop"

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$dest = Join-Path $here "tailwindcss.exe"
$url  = "https://github.com/tailwindlabs/tailwindcss/releases/latest/download/tailwindcss-windows-x64.exe"

Write-Host "Fetching Tailwind CSS standalone CLI..."
Invoke-WebRequest -Uri $url -OutFile $dest
Write-Host "Installed to $dest"
& $dest --help | Select-Object -First 1
