# Screenshots the running Toaster window to a file. Use in dev only.
# Usage: pwsh scripts/dev/screenshot-toaster.ps1 -Out path/to/file.png
param(
  [Parameter(Mandatory=$true)][string]$Out
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms,System.Drawing

$cs = @"
using System;
using System.Runtime.InteropServices;
public class Win32Util {
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hWnd, out RECT rect);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdcBlt, uint nFlags);
}
"@
if (-not ('Win32Util' -as [type])) { Add-Type -TypeDefinition $cs }

$proc = Get-Process toaster -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $proc) { throw "No toaster window found" }

[Win32Util]::ShowWindow($proc.MainWindowHandle, 9) | Out-Null  # SW_RESTORE
Start-Sleep -Milliseconds 400

$rect = New-Object Win32Util+RECT
[void][Win32Util]::GetClientRect($proc.MainWindowHandle, [ref]$rect)
$w = $rect.Right - $rect.Left
$h = $rect.Bottom - $rect.Top
if ($w -le 0 -or $h -le 0) { throw "Invalid client rect ${w}x${h}" }
$bmp = New-Object System.Drawing.Bitmap $w, $h
$g = [System.Drawing.Graphics]::FromImage($bmp)
$hdc = $g.GetHdc()
# PW_CLIENTONLY (0x1) | PW_RENDERFULLCONTENT (0x2) — captures DWM-composited WebView2
$ok = [Win32Util]::PrintWindow($proc.MainWindowHandle, $hdc, 0x3)
$g.ReleaseHdc($hdc)
if (-not $ok) { throw "PrintWindow returned false" }

$dir = Split-Path -Parent $Out
if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
Write-Host "Saved $Out (${w}x${h})"
