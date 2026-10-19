$ErrorActionPreference = 'Stop'
# Isolated loopback servers, synthetic credentials and temporary files only.
# HTTPS S3 fixture needs Git for Windows OpenSSL or STRAJA_TEST_OPENSSL.
& (Join-Path $PSScriptRoot 'cargo.ps1') -CargoArgs @('test', '--locked', '--test', 'remote_destinations', '--', '--include-ignored', '--nocapture')
if ($LASTEXITCODE -ne 0) { throw 'Online destination integration tests failed.' }
