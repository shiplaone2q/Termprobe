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
        (-not $Global:__su_state.HasPSReadLine -or $Global:__su_state.IsInExecution)) {
        $Global:__su_state.IsInExecution = $false
        if ($null -ne $lastHistory -and $lastHistory.Id -ne $Global:__su_state.LastHistoryId) {
            # LASTEXITCODE survives cmdlets and throw. Only use it for a native
            # failure, not for a new PowerShell error from the current command.
            $errorException = if ($lastError -is [System.Management.Automation.ErrorRecord]) {
                $lastError.Exception
            } else {
                $lastError
            }
            $powerShellError = $null -ne $lastError -and
                -not [object]::ReferenceEquals($lastError, $Global:__su_state.LastError) -and
                $lastError.FullyQualifiedErrorId -ne 'NativeCommandError' -and
                ($null -eq $errorException -or $errorException.GetType().FullName -ne 'System.Management.Automation.NativeCommandExitException')
            # Native execution assigns a new boxed exit code, even for repeated
            # failures with the same value. Compare identity so Ignore errors
            # (which do not enter $Error) cannot borrow a previous command's code.
            # This also leaves the user's LASTEXITCODE intact.
