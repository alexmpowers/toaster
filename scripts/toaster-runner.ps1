Set-StrictMode -Version Latest

function Resolve-ToasterPath {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Path,

    [switch]$AllowMissing
  )

  if ([string]::IsNullOrWhiteSpace($Path)) {
    throw "Path must not be empty."
  }

  if ($AllowMissing) {
    return [System.IO.Path]::GetFullPath($Path)
  }

  return (Resolve-Path -LiteralPath $Path -ErrorAction Stop).ProviderPath
}

function Ensure-ToasterParentDirectory {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Path
  )

  $parent = Split-Path -Parent $Path
  if ($parent -and -not (Test-Path -LiteralPath $parent)) {
    New-Item -ItemType Directory -Path $parent -Force | Out-Null
  }
}

function Invoke-ToasterRun {
  [CmdletBinding()]
  param(
    [string]$AppPath = $env:TOASTER_APP_PATH,
    [string]$WorkingDirectory,
    [string]$MediaPath = $env:TOASTER_AUTOMATION_MEDIA,
    [string]$ProjectPath = $env:TOASTER_AUTOMATION_PROJECT,
    [string]$ExportPath = $env:TOASTER_AUTOMATION_EXPORT,
    [string]$WhisperModelPath = $env:TOASTER_WHISPER_MODEL,
    [string]$StdoutPath,
    [string]$StderrPath,
    [switch]$RequireAutomationMedia,
    [string[]]$ExtraArgs = @()
  )

  $skipExitCode = 125
  $repoRoot = Split-Path -Parent $PSScriptRoot
  $originalWhisperModel = $env:TOASTER_WHISPER_MODEL
  $restoreWhisperModel = $false

  try {
    if ([string]::IsNullOrWhiteSpace($AppPath)) {
      $AppPath = Join-Path $repoRoot 'build\bin\toaster-app.exe'
    }

    $resolvedAppPath = Resolve-ToasterPath -Path $AppPath

    if ([string]::IsNullOrWhiteSpace($WorkingDirectory)) {
      $WorkingDirectory = Split-Path -Parent $resolvedAppPath
    } else {
      $WorkingDirectory = Resolve-ToasterPath -Path $WorkingDirectory -AllowMissing
    }

    if (-not (Test-Path -LiteralPath $WorkingDirectory)) {
      New-Item -ItemType Directory -Path $WorkingDirectory -Force | Out-Null
    }

    if ((-not [string]::IsNullOrWhiteSpace($ProjectPath) -or
         -not [string]::IsNullOrWhiteSpace($ExportPath)) -and
        [string]::IsNullOrWhiteSpace($MediaPath)) {
      if ($RequireAutomationMedia) {
        Write-Host 'SKIP: set TOASTER_AUTOMATION_MEDIA to a media file before running the frontend automation hook.'
        return $skipExitCode
      }

      throw 'TOASTER_AUTOMATION_MEDIA is required when TOASTER_AUTOMATION_PROJECT or TOASTER_AUTOMATION_EXPORT is set.'
    }

    $arguments = @()

    if ([string]::IsNullOrWhiteSpace($MediaPath)) {
      if ($RequireAutomationMedia) {
        Write-Host 'SKIP: set TOASTER_AUTOMATION_MEDIA to a media file before running the frontend automation hook.'
        return $skipExitCode
      }
    } else {
      $resolvedMediaPath = Resolve-ToasterPath -Path $MediaPath

      if ([string]::IsNullOrWhiteSpace($ProjectPath)) {
        $ProjectPath = Join-Path $WorkingDirectory 'automation\frontend-smoke.toaster'
      }

      $ProjectPath = Resolve-ToasterPath -Path $ProjectPath -AllowMissing
      Ensure-ToasterParentDirectory -Path $ProjectPath

      $arguments += '--automation-media'
      $arguments += $resolvedMediaPath
      $arguments += '--automation-project'
      $arguments += $ProjectPath

      if (-not [string]::IsNullOrWhiteSpace($ExportPath)) {
        $ExportPath = Resolve-ToasterPath -Path $ExportPath -AllowMissing
        Ensure-ToasterParentDirectory -Path $ExportPath

        $arguments += '--automation-export'
        $arguments += $ExportPath
      }
    }

    if (-not [string]::IsNullOrWhiteSpace($WhisperModelPath)) {
      $WhisperModelPath = Resolve-ToasterPath -Path $WhisperModelPath
      $env:TOASTER_WHISPER_MODEL = $WhisperModelPath
      $restoreWhisperModel = $true
    }

    if (-not [string]::IsNullOrWhiteSpace($StdoutPath)) {
      $StdoutPath = Resolve-ToasterPath -Path $StdoutPath -AllowMissing
      Ensure-ToasterParentDirectory -Path $StdoutPath
      if (Test-Path -LiteralPath $StdoutPath) {
        Remove-Item -LiteralPath $StdoutPath -Force
      }
    }

    if (-not [string]::IsNullOrWhiteSpace($StderrPath)) {
      $StderrPath = Resolve-ToasterPath -Path $StderrPath -AllowMissing
      Ensure-ToasterParentDirectory -Path $StderrPath
      if (Test-Path -LiteralPath $StderrPath) {
        Remove-Item -LiteralPath $StderrPath -Force
      }
    }

    if ($ExtraArgs) {
      $arguments += $ExtraArgs
    }

    $startParameters = @{
      FilePath = $resolvedAppPath
      WorkingDirectory = $WorkingDirectory
      Wait = $true
      PassThru = $true
    }

    if ($arguments.Count -gt 0) {
      $startParameters.ArgumentList = $arguments
    }

    if (-not [string]::IsNullOrWhiteSpace($StdoutPath)) {
      $startParameters.RedirectStandardOutput = $StdoutPath
    }

    if (-not [string]::IsNullOrWhiteSpace($StderrPath)) {
      $startParameters.RedirectStandardError = $StderrPath
    }

    $process = Start-Process @startParameters
    return [int]$process.ExitCode
  } catch {
    Write-Error $_.Exception.Message
    return 1
  } finally {
    if ($restoreWhisperModel) {
      if ([string]::IsNullOrWhiteSpace($originalWhisperModel)) {
        Remove-Item Env:TOASTER_WHISPER_MODEL -ErrorAction SilentlyContinue
      } else {
        $env:TOASTER_WHISPER_MODEL = $originalWhisperModel
      }
    }
  }
}
