$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
Invoke-WebRequest -UseBasicParsing 'https://valorant-api.com/v1/competitivetiers' -OutFile (Join-Path $projectRoot 'crates/core/tests/fixtures/tiers.json')
foreach ($font in @('anton','rajdhani')) {
    $weight = if ($font -eq 'anton') { '400' } else { '600' }
    $baseUrl = "https://cdn.jsdelivr.net/npm/@fontsource/$font"
    Invoke-WebRequest -UseBasicParsing "$baseUrl/files/$font-latin-$weight-normal.woff2" -OutFile (Join-Path $projectRoot "public/fonts/$font.woff2")
    Invoke-WebRequest -UseBasicParsing "$baseUrl/LICENSE" -OutFile (Join-Path $projectRoot "public/fonts/$font-LICENSE.txt")
}
