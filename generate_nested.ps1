# Generates the nested include chain templates/test/nest_0.stpl .. nest_9.stpl
# (10 levels of deep recursive minification).
$ErrorActionPreference = "Stop"
$testDir = Join-Path $PSScriptRoot "templates\test"
New-Item -ItemType Directory -Force -Path $testDir | Out-Null

$utf8NoBom = New-Object System.Text.UTF8Encoding $false

[System.IO.File]::WriteAllText(
    (Join-Path $testDir "nest_0.stpl"),
    '<div>Level 0 <strong>deepest</strong></div>',
    $utf8NoBom
)
for ($i = 1; $i -le 9; $i++) {
    $content = "<section><h2>Level $i</h2><% include!(`"nest_$($i-1).stpl`"); %></section>"
    [System.IO.File]::WriteAllText((Join-Path $testDir "nest_$i.stpl"), $content, $utf8NoBom)
}
Write-Host "Generated templates/test/nest_0.stpl .. nest_9.stpl"
