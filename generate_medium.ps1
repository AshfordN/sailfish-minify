# Generates templates/test/medium.stpl (~100 KB) for performance testing.
$ErrorActionPreference = "Stop"
$testDir = Join-Path $PSScriptRoot "templates\test"
New-Item -ItemType Directory -Force -Path $testDir | Out-Null

$lines = @()
$lines += '<!DOCTYPE html>'
$lines += '<html><head><title>Medium Template - Performance Test</title>'
$lines += '<meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">'
$lines += '<link rel="stylesheet" href="style.css">'
$lines += '<style>'
for ($i = 0; $i -lt 200; $i++) { $lines += "    .cls-$i { margin: ${i}px; padding: ${i}px; color: #$(('{0:X6}' -f $i)); }" }
$lines += '</style></head><body>'
$lines += '<header><nav id="main-nav" class="navbar">'
for ($i = 0; $i -lt 30; $i++) { $lines += "    <a href=`"/page-$i`">Page $i</a>" }
$lines += '</nav></header>'
$lines += '<main><article>'
for ($i = 0; $i -lt 500; $i++) {
    $lines += "    <section id=`"section-$i`" class=`"content-block`"><h2>Section $i</h2>"
    $lines += "    <p>This is section $i content with <strong>bold</strong>, <em>italic</em>, and <a href=`"#section-$($i+1)`">links</a>.</p>"
    $lines += "    <ul>"
    for ($j = 0; $j -lt 5; $j++) { $lines += "        <li>Item $i-$j: some descriptive text here for testing purposes</li>" }
    $lines += "    </ul></section>"
}
$lines += '</article></main>'
$lines += '<footer><p>Generated for sailfish-minify performance testing.</p></footer>'
$lines += '</body></html>'
[System.IO.File]::WriteAllText(
    (Join-Path $testDir "medium.stpl"),
    ($lines -join "`n"),
    (New-Object System.Text.UTF8Encoding $false)
)
Write-Host "Generated templates/test/medium.stpl ($((Get-Item (Join-Path $testDir 'medium.stpl')).Length) bytes)"
