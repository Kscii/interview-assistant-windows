$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$installation = & $vswhere -latest -products "*" -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (!$installation) { throw "Visual Studio C++ Build Tools are required." }
$crt = Get-ChildItem "$installation\VC\Redist\MSVC\*\x64\Microsoft.VC*.CRT" -Directory |
    Sort-Object FullName -Descending | Select-Object -First 1
if (!$crt) { throw "No x64 Visual C++ redistributable directory found." }
$dest = Join-Path $root "src-tauri\redistributables"
New-Item $dest -ItemType Directory -Force | Out-Null
$files = @()
foreach ($file in (Get-ChildItem $crt.FullName -Filter "*.dll")) {
    Copy-Item $file.FullName $dest -Force
    $files += @{
        name = $file.Name
        version = $file.VersionInfo.FileVersion
        sha256 = (Get-FileHash $file.FullName -Algorithm SHA256).Hash.ToLower()
    }
}
foreach ($required in @("msvcp140.dll", "msvcp140_1.dll", "vcruntime140.dll", "vcruntime140_1.dll")) {
    if (!(Test-Path (Join-Path $dest $required))) { throw "Missing runtime: $required" }
}
@{ source = $crt.Name; files = $files } | ConvertTo-Json -Depth 5 |
    Set-Content (Join-Path $dest "msvc-runtime-manifest.json") -Encoding UTF8
Write-Host "Prepared $($files.Count) application-local Visual C++ runtime libraries."
