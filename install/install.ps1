$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repository = "microsoft/termprobe"
$binaryName = "termprobe.exe"

if ($env:OS -ne "Windows_NT") {
    throw "install.ps1 only supports Windows. Use install.sh on macOS or Linux."
}

$processorArchitecture = $env:PROCESSOR_ARCHITEW6432
if ([string]::IsNullOrWhiteSpace($processorArchitecture)) {
    $processorArchitecture = $env:PROCESSOR_ARCHITECTURE
}

switch ($processorArchitecture.ToUpperInvariant()) {
    "AMD64" { $architecture = "x86_64" }
    "X86_64" { $architecture = "x86_64" }
    "ARM64" { $architecture = "aarch64" }
    "AARCH64" { $architecture = "aarch64" }
    default { throw "Unsupported Windows architecture: $processorArchitecture" }
}

$target = "$architecture-pc-windows-msvc"
$asset = "termprobe-$target.zip"
$version = $env:TERMPROBE__VERSION
$token = $env:GITHUB_TOKEN
if ([string]::IsNullOrWhiteSpace($token)) {
    $token = $env:GH_TOKEN
}

if ([string]::IsNullOrWhiteSpace($version) -or $version -eq "latest") {
    $releaseUrl = "https://github.com/$repository/releases/latest/download"
}
elseif ($version -eq "beta") {
    $headers = @{
        Accept = "application/vnd.github+json"
        "X-GitHub-Api-Version" = "2022-11-28"
    }
    if (-not [string]::IsNullOrWhiteSpace($token)) {
        $headers["Authorization"] = "Bearer $token"
    }

    $releases = Invoke-RestMethod `
        -Uri "https://api.github.com/repos/$repository/releases?per_page=100" `
        -Headers $headers `
        -UseBasicParsing
    $latestBeta = $releases |
        Where-Object {
            $_.prerelease -and
            -not $_.draft -and
            $_.tag_name -match "^[0-9]+\.[0-9]+\.[0-9]+-beta\.[0-9]+$"
        } |
        Select-Object -First 1
    if ($null -eq $latestBeta -or [string]::IsNullOrWhiteSpace($latestBeta.tag_name)) {
        throw "No beta release was found."
    }

    $version = $latestBeta.tag_name
    $releaseUrl = "https://github.com/$repository/releases/download/$version"
}
else {
    $releaseUrl = "https://github.com/$repository/releases/download/$version"
}

$downloadUrl = "$releaseUrl/$asset"

$tempDir = Join-Path ([IO.Path]::GetTempPath()) ("termprobe-" + [Guid]::NewGuid())
$archivePath = Join-Path $tempDir $asset
$extractDir = Join-Path $tempDir "extract"

New-Item -ItemType Directory -Path $extractDir -Force | Out-Null

try {
    $request = @{
