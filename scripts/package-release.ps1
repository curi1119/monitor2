param([Parameter(Mandatory)][ValidatePattern('^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$')][string]$Version)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$distPath = Join-Path $repoRoot 'target/dist'
# A unique staging directory prevents files from an earlier build entering the ZIP.
$stagePath = Join-Path $distPath ([guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $stagePath -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $repoRoot 'target/release/monitor2.exe') -Destination $stagePath
foreach ($file in @('LICENSE', 'THIRD_PARTY_NOTICES.md')) {
    Copy-Item -LiteralPath (Join-Path $repoRoot $file) -Destination $stagePath
}
Copy-Item -LiteralPath (Join-Path $repoRoot 'docs/settings.md') -Destination (Join-Path $stagePath '使用方法.md')
Copy-Item -LiteralPath (Join-Path $repoRoot 'assets/brands/LICENSE-CC0.txt') -Destination $stagePath
Copy-Item -LiteralPath (Join-Path $repoRoot 'assets/brands/DISCLAIMER.md') -Destination $stagePath
Copy-Item -LiteralPath (Join-Path $repoRoot 'assets/licenses/windows-rs-MIT.txt') -Destination $stagePath
$zipPath = Join-Path $distPath "monitor2-$Version-windows-x64.zip"
Compress-Archive -Path (Join-Path $stagePath '*') -DestinationPath $zipPath -Force
$hash = (Get-FileHash -LiteralPath $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()
[IO.File]::WriteAllText("$zipPath.sha256", "$hash  $([IO.Path]::GetFileName($zipPath))`n", [Text.UTF8Encoding]::new($false))
Write-Output $zipPath
