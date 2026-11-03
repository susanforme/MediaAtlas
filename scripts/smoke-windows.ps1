$ErrorActionPreference = 'Stop'
$executable = Join-Path $PSScriptRoot '../target/release/mediaatlas-desktop.exe'
if (-not (Test-Path $executable)) {
  throw "Desktop executable was not built: $executable"
}
$process = Start-Process -FilePath $executable -PassThru
try {
  Start-Sleep -Seconds 10
  $process.Refresh()
  if ($process.HasExited) {
    throw "Desktop exited during startup with code $($process.ExitCode)"
  }
  Write-Output 'Desktop process remained running for 10 seconds. Visual window/IPC verification is a separate manual check.'
} finally {
  $process.Refresh()
  if (-not $process.HasExited) {
    Stop-Process -Id $process.Id -Force
  }
}
