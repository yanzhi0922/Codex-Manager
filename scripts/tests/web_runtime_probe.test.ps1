$ErrorActionPreference = "Stop"

$scriptPath = Join-Path $PSScriptRoot "web_runtime_probe.ps1"
if (-not (Test-Path $scriptPath -PathType Leaf)) {
  throw "missing web_runtime_probe.ps1 at $scriptPath"
}

$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("codex_web_runtime_probe_" + [Guid]::NewGuid().ToString("N"))
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
  $runtimeJsonPath = Join-Path $tempRoot "runtime.json"
  $initializeJsonPath = Join-Path $tempRoot "initialize.json"

  Write-Utf8File -Path $runtimeJsonPath -Content @"
{
  "mode": "web-gateway",
  "rpcBaseUrl": "/api/rpc",
  "canManageService": false,
  "canSelfUpdate": false,
  "canCloseToTray": false,
  "canOpenLocalDir": false
}
"@

  Write-Utf8File -Path $initializeJsonPath -Content @"
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "serverName": "codexmanager-service",
    "version": "0.1.15"
  }
}
"@

  $result = & $scriptPath -Base "http://localhost:48761" -RuntimeJsonPath $runtimeJsonPath -InitializeJsonPath $initializeJsonPath
  if (-not $?) {
    throw "web_runtime_probe.ps1 synthetic run failed"
  }
  if ($result.Mode -ne "web-gateway") {
    throw "expected web-gateway mode from web_runtime_probe.ps1"
  }
  if ($result.ServiceName -ne "codexmanager-service") {
    throw "expected service name output from web_runtime_probe.ps1"
  }
  if ($result.RpcUrl -ne "http://localhost:48761/api/rpc") {
    throw "expected resolved rpc url output from web_runtime_probe.ps1"
  }

  Write-Host "web_runtime_probe.ps1 synthetic runtime check looks ok"
} finally {
  if (Test-Path $tempRoot) {
    Remove-Item -Recurse -Force $tempRoot
  }
}
