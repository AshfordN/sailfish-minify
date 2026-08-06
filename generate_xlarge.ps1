# Generates templates/test/xlarge.stpl (~10 MB) for stress-testing the pipeline.
$ErrorActionPreference = "Stop"
$testDir = Join-Path $PSScriptRoot "templates\test"
New-Item -ItemType Directory -Force -Path $testDir | Out-Null

$lines = @('<!DOCTYPE html><html><head><title>XL Template</title></head><body>')
for ($i = 0; $i -lt 50000; $i++) {
    $lines += "<div id=`"block$i`"><p>Paragraph $i with enough text to make this template very large for stress-testing the minifier pipeline. Lorem ipsum dolor sit amet consectetur adipiscing elit.</p></div>"
}
$lines += '</body></html>'
[System.IO.File]::WriteAllText(
    (Join-Path $testDir "xlarge.stpl"),
    ($lines -join "`n"),
    (New-Object System.Text.UTF8Encoding $false)
)
Write-Host "Generated templates/test/xlarge.stpl ($((Get-Item (Join-Path $testDir 'xlarge.stpl')).Length) bytes)"
