param([ValidateSet('wgpu','tiny-skia')][string]$Renderer = 'wgpu')
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$fixture = Join-Path $projectRoot 'test-output\straja-scroll-preview-0.5.2.exe'
if (!(Test-Path -LiteralPath $fixture)) { throw 'Build the optimized ui-preview fixture first; see PERFORMANCE.md.' }
$report = Join-Path $projectRoot ("test-output\scroll-$Renderer-" + [Guid]::NewGuid().ToString('N') + '.json')
$oldBackend = $env:ICED_BACKEND
try {
    # Only this child receives the backend override; no system/user environment is changed.
    $env:ICED_BACKEND = $Renderer
    $probe = Start-Process -FilePath $fixture -ArgumentList @('--preview','--scroll-probe',('"' + $report + '"')) -WindowStyle Hidden -PassThru
} finally { $env:ICED_BACKEND = $oldBackend }
$deadline = [DateTime]::UtcNow.AddSeconds(120)
$peakWorkingSet = 0L
$peakPrivate = 0L
Write-Host "Isolated scroll fixture PID $($probe.Id), renderer $Renderer"
while (!$probe.HasExited) {
    $probe.Refresh()
    if ($probe.HasExited) { break }
    $peakWorkingSet = [Math]::Max($peakWorkingSet, $probe.WorkingSet64)
    $peakPrivate = [Math]::Max($peakPrivate, $probe.PrivateMemorySize64)
    if ([DateTime]::UtcNow -ge $deadline) {
        if ($probe.Path -eq $fixture) { Stop-Process -Id $probe.Id }
        throw 'The isolated scroll fixture timed out.'
    }
    Start-Sleep -Milliseconds 200
}
if ($probe.ExitCode -ne 0 -or !(Test-Path -LiteralPath $report)) { throw "Scroll fixture failed: $($probe.ExitCode)" }
$result = Get-Content -LiteralPath $report -Raw | ConvertFrom-Json
$result | Add-Member -NotePropertyName sampled_peak_working_set_mib -NotePropertyValue ($peakWorkingSet / 1MB)
$result | Add-Member -NotePropertyName sampled_peak_private_mib -NotePropertyValue ($peakPrivate / 1MB)
$result | Add-Member -NotePropertyName total_process_cpu_seconds -NotePropertyValue $probe.TotalProcessorTime.TotalSeconds
$result | ConvertTo-Json | Set-Content -LiteralPath $report -Encoding utf8
Get-Content -LiteralPath $report
Write-Host "Report: $report"
