# Generates templates/test/big_whitespace.stpl (~1 MB of whitespace-heavy HTML)
# used by test_big_whitespace_template_* in tests/integration_tests.rs.
# The fixture is committed so the tests work on a fresh checkout.
$ErrorActionPreference = "Stop"
$testDir = Join-Path $PSScriptRoot "templates\test"
New-Item -ItemType Directory -Force -Path $testDir | Out-Null

$utf8NoBom = New-Object System.Text.UTF8Encoding $false
$sb = New-Object System.Text.StringBuilder

# Header
[void]$sb.AppendLine('<!DOCTYPE html>')
[void]$sb.AppendLine('<html lang="en">')
[void]$sb.AppendLine('<head>')
[void]$sb.AppendLine('    <meta charset="utf-8">')
[void]$sb.AppendLine('    <meta name="viewport" content="width=device-width, initial-scale=1">')
[void]$sb.AppendLine('    <title><%= title %></title>')
[void]$sb.AppendLine('</head>')
[void]$sb.AppendLine('')
[void]$sb.AppendLine('<body>')
[void]$sb.AppendLine('    <main id="content">')
[void]$sb.AppendLine('')

for ($i = 0; $i -lt 1000; $i++) {
    [void]$sb.AppendLine('        <section id="section-' + $i + '" class="content-block" data-index="' + $i + '">')
    [void]$sb.AppendLine('')
    [void]$sb.AppendLine('            <h2 class="section-title">Section Heading ' + $i + '</h2>')
    [void]$sb.AppendLine('')
    [void]$sb.AppendLine('')
    [void]$sb.AppendLine('            <p class="intro">')
    [void]$sb.AppendLine('')
    [void]$sb.AppendLine('                This is the introductory paragraph for section ' + $i + '. It carries a')
    [void]$sb.AppendLine('                reasonable amount of descriptive text so the template has real content')
    [void]$sb.AppendLine('                for the minifier to work with, spread across several lines with plenty')
    [void]$sb.AppendLine('                of leading whitespace to collapse away during minification.')
    [void]$sb.AppendLine('')
    [void]$sb.AppendLine('            </p>')
    [void]$sb.AppendLine('')
    [void]$sb.AppendLine('')
    [void]$sb.AppendLine('            <ul class="items">')
    [void]$sb.AppendLine('')
    for ($j = 0; $j -lt 6; $j++) {
        [void]$sb.AppendLine('                <li>Item ' + $i + '-' + $j + ' : a descriptive line of text for this list entry</li>')
    }
    [void]$sb.AppendLine('')
    [void]$sb.AppendLine('            </ul>')
    [void]$sb.AppendLine('')
    [void]$sb.AppendLine('')
    [void]$sb.AppendLine('            <p class="outro">')
    [void]$sb.AppendLine('                <a href="#section-' + ($i + 1) + '">Next section</a> — closing text for section ' + $i + ' continues here.')
    [void]$sb.AppendLine('            </p>')
    [void]$sb.AppendLine('')
    [void]$sb.AppendLine('')
    [void]$sb.AppendLine('        </section>')
    [void]$sb.AppendLine('')
    [void]$sb.AppendLine('')
}

[void]$sb.AppendLine('    </main>')
[void]$sb.AppendLine('')
[void]$sb.AppendLine('    <footer id="UNIQUE_MARKER_FOOTER">')
[void]$sb.AppendLine('        <p>Footer for <%= title %> — this text must survive minification.</p>')
[void]$sb.AppendLine('    </footer>')
[void]$sb.AppendLine('')
[void]$sb.AppendLine('</body>')
[void]$sb.AppendLine('</html>')

[System.IO.File]::WriteAllText((Join-Path $testDir "big_whitespace.stpl"), $sb.ToString(), $utf8NoBom)
Write-Host "Generated big_whitespace.stpl ($((Get-Item (Join-Path $testDir 'big_whitespace.stpl')).Length) bytes)"
