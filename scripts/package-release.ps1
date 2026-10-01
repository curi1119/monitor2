param([Parameter(Mandatory)][ValidatePattern('^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$')][string]$Version)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$distPath = Join-Path $repoRoot 'target/dist'
# A unique staging directory prevents files from an earlier build entering the ZIP.
$stagePath = Join-Path $distPath ([guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $stagePath -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $repoRoot 'target/release/monitor2.exe') -Destination $stagePath
# Keep required MIT notices while avoiding separate third-party document files.
$utf8 = [Text.UTF8Encoding]::new($false)
$appLicense = [IO.File]::ReadAllText((Join-Path $repoRoot 'LICENSE')).TrimEnd()
$windowsLicense = [IO.File]::ReadAllText((Join-Path $repoRoot 'assets/licenses/windows-rs-MIT.txt')).TrimEnd()
$combinedLicense = "monitor2 software (MIT)`n`n$appLicense`n`n---`n`nwindows-sys / windows-link (Microsoft, MIT)`n`n$windowsLicense`n"
[IO.File]::WriteAllText((Join-Path $stagePath 'LICENSE'), $combinedLicense.Replace("`r`n", "`n"), $utf8)
Copy-Item -LiteralPath (Join-Path $repoRoot 'docs/release-readme.txt') -Destination (Join-Path $stagePath 'readme.txt')
$zipPath = Join-Path $distPath "monitor2-$Version-windows-x64.zip"
Compress-Archive -Path (Join-Path $stagePath '*') -DestinationPath $zipPath -Force
$hash = (Get-FileHash -LiteralPath $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()
[IO.File]::WriteAllText("$zipPath.sha256", "$hash  $([IO.Path]::GetFileName($zipPath))`n", [Text.UTF8Encoding]::new($false))
Write-Output $zipPath
