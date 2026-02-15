# RTK auto-rewrite wrapper for PowerShell PreToolUse hooks
# Reads hook JSON from stdin and forwards it to the bash implementation.

$bash = Get-Command bash -ErrorAction SilentlyContinue
if (-not $bash) {
    exit 0
}

$inputJson = [Console]::In.ReadToEnd()
if ([string]::IsNullOrWhiteSpace($inputJson)) {
    exit 0
}

$inputJson | & bash "./.github/hooks/rtk-rewrite.sh"
exit $LASTEXITCODE
