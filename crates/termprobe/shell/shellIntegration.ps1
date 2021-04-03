if ((Test-Path variable:global:__su_state) -and $null -ne $Global:__su_state.OriginalPrompt) {
    return
}

if ($ExecutionContext.SessionState.LanguageMode -ne "FullLanguage") {
    return
}

$Global:__su_state = @{
    OriginalPrompt = $function:Prompt
    LastHistoryId  = -1
    IsInExecution  = $false
    HasPSReadLine  = $false
    LastError      = $null
    LastNativeExit = $null
}

function Global:__su_seq([string]$m) { return "$([char]0x1b)]$m$([char]0x07)" }

function Global:Prompt() {
    # Capture $? before invoking anything.
    $ok = $global:?

    # LASTEXITCODE may not exist yet; Get-History may return $null.
    Set-StrictMode -Off
    $lastExit = $global:LASTEXITCODE
    $lastError = $global:Error[0]
    $lastHistory = Get-History -Count 1

    $out = ""
    if ($Global:__su_state.LastHistoryId -ne -1 -and
