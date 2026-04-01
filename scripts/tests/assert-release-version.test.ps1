$ErrorActionPreference = "Stop"

$scriptPath = Join-Path (Split-Path -Parent $PSScriptRoot) "release/assert-release-version.ps1"
if (-not (Test-Path $scriptPath -PathType Leaf)) {
  throw "missing assert-release-version.ps1 at $scriptPath"
}

$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$currentVersion = ((Get-Content (Join-Path $repoRoot "apps/src-tauri/tauri.conf.json") -Raw) | ConvertFrom-Json).version

& $scriptPath -Tag "v$currentVersion" | Out-Null
if (-not $?) {
  throw "assert-release-version.ps1 should pass on current workspace"
}

$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("assert_release_version_" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $tempRoot | Out-Null

function Write-Utf8File {
  param(
    [string]$Path,
    [string]$Content
  )
  $dir = Split-Path -Parent $Path
  if ($dir -and -not (Test-Path $dir)) {
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
  }
  $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
  [System.IO.File]::WriteAllText($Path, $Content, $utf8NoBom)
}

try {
  $rootCargo = Join-Path $tempRoot "Cargo.toml"
  $tauriCargo = Join-Path $tempRoot "apps/src-tauri/Cargo.toml"
  $tauriConfig = Join-Path $tempRoot "apps/src-tauri/tauri.conf.json"
  $frontendPackage = Join-Path $tempRoot "apps/package.json"
  $cratesRoot = Join-Path $tempRoot "crates"
  $coreCargo = Join-Path $cratesRoot "core/Cargo.toml"
  $serviceCargo = Join-Path $cratesRoot "service/Cargo.toml"

  Write-Utf8File -Path $rootCargo -Content @"
[workspace]
members = ["crates/core", "crates/service"]

[workspace.package]
version = "0.9.9"
"@

  Write-Utf8File -Path $tauriCargo -Content @"
[package]
name = "CodexManager"
version = "0.9.9"
edition = "2021"
"@

  Write-Utf8File -Path $tauriConfig -Content @"
{
  "version": "0.9.9"
}
"@

  Write-Utf8File -Path $frontendPackage -Content @"
{
  "name": "codexmanager-frontend",
  "version": "0.9.9"
}
"@

  Write-Utf8File -Path $coreCargo -Content @"
[package]
name = "codexmanager-core"
version.workspace = true
edition = "2021"
"@

  Write-Utf8File -Path $serviceCargo -Content @"
[package]
name = "codexmanager-service"
version = "0.9.9"
edition = "2021"
"@

  & $scriptPath -Tag "v0.9.9" -RootCargoTomlPath $rootCargo -CargoTomlPath $tauriCargo -TauriConfigPath $tauriConfig -FrontendPackageJsonPath $frontendPackage -WorkspaceCratesRoot $cratesRoot | Out-Null
  if (-not $?) {
    throw "assert-release-version.ps1 should pass on aligned versions"
  }

  Write-Utf8File -Path $serviceCargo -Content @"
[package]
name = "codexmanager-service"
version = "1.0.0"
edition = "2021"
"@

  $failed = $false
  try {
    & $scriptPath -Tag "v0.9.9" -RootCargoTomlPath $rootCargo -CargoTomlPath $tauriCargo -TauriConfigPath $tauriConfig -FrontendPackageJsonPath $frontendPackage -WorkspaceCratesRoot $cratesRoot | Out-Null
  } catch {
    $failed = $_.Exception.Message -like "*workspace crate version mismatch*"
  }
  if (-not $failed) {
    throw "expected workspace crate mismatch to fail"
  }

  Write-Utf8File -Path $serviceCargo -Content @"
[package]
name = "codexmanager-service"
version = "0.9.9"
edition = "2021"
"@

  Write-Utf8File -Path $frontendPackage -Content @"
{
  "name": "codexmanager-frontend",
  "version": "1.0.0"
}
"@

  $frontendFailed = $false
  try {
    & $scriptPath -Tag "v0.9.9" -RootCargoTomlPath $rootCargo -CargoTomlPath $tauriCargo -TauriConfigPath $tauriConfig -FrontendPackageJsonPath $frontendPackage -WorkspaceCratesRoot $cratesRoot | Out-Null
  } catch {
    $frontendFailed = $_.Exception.Message -like "*version mismatch*" -and $_.Exception.Message -match 'apps[\\/]+package\.json'
  }
  if (-not $frontendFailed) {
    throw "expected frontend package version mismatch to fail"
  }

  Write-Host "assert-release-version.ps1 checks current workspace and synthetic workspace alignment"
} finally {
  if (Test-Path $tempRoot) {
    Remove-Item -Recurse -Force $tempRoot
  }
}
