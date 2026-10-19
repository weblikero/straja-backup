$ErrorActionPreference = 'Stop'
& (Join-Path $PSScriptRoot 'cargo.ps1') test --locked --all-targets
if ($LASTEXITCODE -ne 0) { throw 'Testele unitare au eșuat.' }
& (Join-Path $PSScriptRoot 'cargo.ps1') -CargoArgs @('test', '--locked', '--test', 'restic_roundtrip', '--', '--ignored', '--nocapture')
if ($LASTEXITCODE -ne 0) { throw 'Testul real Restic a eșuat.' }
& (Join-Path $PSScriptRoot 'cargo.ps1') -CargoArgs @('test', '--locked', '--test', 'worker_lifecycle', '--', '--ignored', '--nocapture')
if ($LASTEXITCODE -ne 0) { throw 'Testul agentului și programării a eșuat.' }

& (Join-Path $PSScriptRoot 'cargo.ps1') -CargoArgs @('test', '--locked', '--test', 'selected_exclusions', '--', '--ignored', '--nocapture')
if ($LASTEXITCODE -ne 0) { throw 'Testul excluderilor selectate cu Restic a eșuat.' }
