$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$installer = Get-ChildItem "$root\target\release\bundle\nsis\*.exe" | Select-Object -First 1
if (!$installer) { throw "Installer not found." }
$dest = Join-Path $root "artifacts\installed"
$setup = Start-Process $installer.FullName -ArgumentList @("/S", "/D=$dest") -Wait -PassThru
if ($setup.ExitCode -ne 0) { throw "Installer exited with $($setup.ExitCode)" }
foreach ($file in @("interview-assistant.exe", "msvcp140.dll", "msvcp140_1.dll", "vcruntime140.dll", "vcruntime140_1.dll",
    "resources\models\det.onnx", "resources\models\rec.onnx", "resources\models\keys.txt",
    "resources\runtime\onnxruntime.dll", "resources\licenses\index.json")) {
    if (!(Test-Path (Join-Path $dest $file))) { throw "Installed file missing: $file" }
}
$report = Join-Path $root "artifacts\installed-ocr.json"
$fixture = Join-Path $root "tests\fixtures\question.png"
$check = Start-Process (Join-Path $dest "interview-assistant.exe") -ArgumentList @("--check-ocr", "`"$fixture`"", "`"$report`"") -Wait -PassThru
if ($check.ExitCode -ne 0) {
    if (Test-Path $report) { Get-Content $report }
    throw "Installed OCR self-check exited with $($check.ExitCode)"
}
$result = Get-Content $report -Raw | ConvertFrom-Json
if (!$result.ok -or $result.text -notmatch "Tell me about yourself" -or $result.text -notmatch "project") {
    throw "Installed OCR did not recognize the fixture: $($result.text)"
}
Write-Host "Installer and installed CPU OCR verified in $($result.elapsed_ms) ms."
