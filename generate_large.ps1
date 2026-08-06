# Generates templates/test/large.stpl (~1 MB) for benchmarking.
$ErrorActionPreference = "Stop"
$testDir = Join-Path $PSScriptRoot "templates\test"
New-Item -ItemType Directory -Force -Path $testDir | Out-Null

$lines = @('<!DOCTYPE html><html lang="en"><head><title>Large Template - Benchmark</title>')
$lines += '<meta charset="utf-8"><style>'
for ($i = 0; $i -lt 1000; $i++) { $lines += ".cls-$i{color:#$('{0:X6}' -f $i)}" }
$lines += '</style></head><body>'
$lines += '<main>'
for ($i = 0; $i -lt 3000; $i++) {
    $lines += "<div id=`"d$i`" class=`"container`">"
    $lines += "<h2>Block $i</h2>"
    $lines += "<p>Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.</p>"
    $lines += "<table class=`"data-table`"><thead><tr>"
    for ($j = 0; $j -lt 8; $j++) { $lines += "<th>Column $j</th>" }
    $lines += "</tr></thead><tbody>"
    for ($k = 0; $k -lt 5; $k++) {
        $lines += "<tr>"
        for ($j = 0; $j -lt 8; $j++) { $lines += "<td>Row $k Col $j</td>" }
        $lines += "</tr>"
    }
    $lines += "</tbody></table></div>"
}
$lines += '</main></body></html>'
[System.IO.File]::WriteAllText(
    (Join-Path $testDir "large.stpl"),
    ($lines -join "`n"),
    (New-Object System.Text.UTF8Encoding $false)
)
Write-Host "Generated templates/test/large.stpl ($((Get-Item (Join-Path $testDir 'large.stpl')).Length) bytes)"
