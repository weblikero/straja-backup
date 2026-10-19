#Requires -RunAsAdministrator
param([Parameter(Mandatory=$true)][string]$TestExecutable, [Parameter(Mandatory=$true)][string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Split-Path $PSScriptRoot -Parent))
$testPath = (Resolve-Path -LiteralPath $TestExecutable).Path
$allowedTests = [IO.Path]::GetFullPath((Join-Path $projectRoot 'target\verification\debug\deps')) + '\'
if (!$testPath.StartsWith($allowedTests, [StringComparison]::OrdinalIgnoreCase) -or
    (Split-Path $testPath -Leaf) -notmatch '^vss-[a-f0-9]+\.exe$') { throw 'Expected the compiled VSS integration test executable in target/verification/debug/deps.' }
$output = [IO.Path]::GetFullPath($OutputDirectory)
$allowedOutput = [IO.Path]::GetFullPath((Join-Path $projectRoot 'test-output')) + '\'
if (!$output.StartsWith($allowedOutput, [StringComparison]::OrdinalIgnoreCase)) { throw 'Output must be inside this project test-output directory.' }
New-Item -ItemType Directory -Path $output -Force | Out-Null
# Test data and logs only. No installed agent/service, user plan or backup is touched.
$env:TEMP = Join-Path $output 'temp'
$env:TMP = $env:TEMP
New-Item -ItemType Directory -Path $env:TEMP -Force | Out-Null
$code = 1
try {
    & $testPath --ignored --nocapture --test-threads=1 2>&1 | Tee-Object -FilePath (Join-Path $output 'tests.log')
    $code = $LASTEXITCODE
} catch {
    $_ | Out-String | Add-Content -LiteralPath (Join-Path $output 'tests.log')
} finally {
    [pscustomobject]@{ ExitCode=$code; FinishedAt=[DateTime]::UtcNow.ToString('o'); TestExecutable=$testPath } |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $output 'result.json') -Encoding utf8
}
exit $code
