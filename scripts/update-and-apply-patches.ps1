[CmdletBinding()]
param(
    [string]$UpstreamUrl = "https://github.com/QingJ01/Pebble.git",
    [string]$Remote = "upstream",
    [string]$Branch = "master",
    [switch]$Push
)

$ErrorActionPreference = "Stop"

function Invoke-Git {
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Arguments)

    & git @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "git $($Arguments -join ' ') failed with exit code $LASTEXITCODE"
    }
}

function Test-Git {
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Arguments)

    & git @Arguments *> $null
    return $LASTEXITCODE -eq 0
}

function Set-ActionOutput {
    param(
        [string]$Name,
        [string]$Value
    )

    if ($env:GITHUB_OUTPUT) {
        "$Name=$Value" | Out-File -FilePath $env:GITHUB_OUTPUT -Append -Encoding utf8
    }
}

$repoRoot = (& git rev-parse --show-toplevel).Trim()
if ($LASTEXITCODE -ne 0 -or -not $repoRoot) {
    throw "This script must run inside a git repository."
}
Set-Location $repoRoot

$dirty = (& git status --porcelain)
if ($dirty) {
    throw "Working tree must be clean before syncing upstream."
}

$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) "pebble-patches-$([guid]::NewGuid().ToString('N'))"
$managedPaths = @(
    ".gitattributes",
    "patches",
    "scripts/update-and-apply-patches.ps1",
    ".github/workflows/auto-patch.yml"
)

try {
    New-Item -ItemType Directory -Force $tempRoot | Out-Null

    foreach ($path in $managedPaths) {
        if (-not (Test-Path -LiteralPath $path)) {
            continue
        }
        $dest = Join-Path $tempRoot $path
        $parent = Split-Path -Parent $dest
        if ($parent) {
            New-Item -ItemType Directory -Force $parent | Out-Null
        }
        Copy-Item -LiteralPath $path -Destination $dest -Recurse -Force
    }

    if (Test-Git remote get-url $Remote) {
        Invoke-Git remote set-url $Remote $UpstreamUrl
    } else {
        Invoke-Git remote add $Remote $UpstreamUrl
    }

    Invoke-Git fetch $Remote $Branch

    $currentHead = (& git rev-parse HEAD).Trim()
    $upstreamHead = (& git rev-parse "$Remote/$Branch").Trim()
    Set-ActionOutput "current_head" $currentHead
    Set-ActionOutput "upstream_head" $upstreamHead

    if (Test-Git merge-base --is-ancestor $upstreamHead HEAD) {
        Write-Host "No upstream updates found. Current branch already contains $Remote/$Branch."
        Set-ActionOutput "upstream_changed" "false"
        Set-ActionOutput "commit_created" "false"
        Set-ActionOutput "pushed" "false"
        Set-ActionOutput "patched_head" $currentHead
        return
    }

    Set-ActionOutput "upstream_changed" "true"
    Invoke-Git reset --hard "$Remote/$Branch"
    Invoke-Git clean -fd

    $patchDir = Join-Path $tempRoot "patches"
    if (-not (Test-Path -LiteralPath $patchDir)) {
        throw "No patches directory found."
    }

    $patches = Get-ChildItem -LiteralPath $patchDir -Filter "*.patch" | Sort-Object Name
    if (-not $patches) {
        throw "No patch files found in patches/."
    }

    $utf8NoBom = New-Object System.Text.UTF8Encoding $false

    foreach ($patch in $patches) {
        $normalizedPatch = Join-Path $tempRoot "normalized-$($patch.Name)"
        $patchText = [System.IO.File]::ReadAllText($patch.FullName) -replace "`r`n", "`n"
        [System.IO.File]::WriteAllText($normalizedPatch, $patchText, $utf8NoBom)

        if (Test-Git apply --check --index $normalizedPatch) {
            Invoke-Git apply --index $normalizedPatch
            Write-Host "Applied patch $($patch.Name)"
            continue
        }

        if (Test-Git apply --reverse --check $normalizedPatch) {
            Write-Host "Patch $($patch.Name) is already present upstream; skipping"
            continue
        }

        throw "Patch does not apply cleanly: $($patch.Name)"
    }

    foreach ($path in $managedPaths) {
        $src = Join-Path $tempRoot $path
        if (-not (Test-Path -LiteralPath $src)) {
            continue
        }
        $parent = Split-Path -Parent (Join-Path $repoRoot $path)
        if ($parent) {
            New-Item -ItemType Directory -Force $parent | Out-Null
        }
        Copy-Item -LiteralPath $src -Destination (Join-Path $repoRoot $path) -Recurse -Force
    }

    Invoke-Git add .gitattributes patches scripts/update-and-apply-patches.ps1 .github/workflows/auto-patch.yml

    if (Test-Git diff --cached --quiet) {
        Write-Host "No patch changes to commit."
        Set-ActionOutput "commit_created" "false"
    } else {
        Invoke-Git commit -m "chore: sync upstream and apply local patches"
        Set-ActionOutput "commit_created" "true"
    }

    $patchedHead = (& git rev-parse HEAD).Trim()
    Set-ActionOutput "patched_head" $patchedHead

    if ($Push) {
        Invoke-Git push --force-with-lease origin "HEAD:$Branch"
        Set-ActionOutput "pushed" "true"
    } else {
        Set-ActionOutput "pushed" "false"
    }
} finally {
    if (Test-Path -LiteralPath $tempRoot) {
        Remove-Item -LiteralPath $tempRoot -Recurse -Force
    }
}
