param([string]$Executable = 'target/release/codebeat.exe')
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)
New-Item -ItemType Directory -Force docs/logs | Out-Null
$exe = (Resolve-Path -LiteralPath $Executable).Path
# Opens the same popup used by tray clicks, without synthesizing desktop input or model requests.
$process = Start-Process -FilePath $exe -ArgumentList '--open-tray' -WindowStyle Hidden -PassThru -RedirectStandardOutput docs/logs/tray-desktop-stdout.txt -RedirectStandardError docs/logs/tray-desktop-stderr.txt
Start-Sleep -Seconds 8
$process.Refresh()
Add-Type @'
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class CodeBeatTrayProbe {
 public class WindowInfo { public string Title; public bool Visible; public int Width,Height; public uint Dpi; public bool Topmost,ToolWindow; }
 [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left,Top,Right,Bottom; }
 delegate bool EnumProc(IntPtr h,IntPtr p);
 [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc callback,IntPtr p);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h,out uint pid);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr h,StringBuilder text,int count);
 [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")] static extern bool GetClientRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")] static extern uint GetDpiForWindow(IntPtr h);
 [DllImport("user32.dll",EntryPoint="GetWindowLongPtrW")] static extern IntPtr GetWindowLongPtr(IntPtr h,int index);
 [DllImport("user32.dll")] static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
 public static WindowInfo[] Inspect(uint pid) {
  var rows=new List<WindowInfo>();var previous=SetThreadDpiAwarenessContext(new IntPtr(-4));
  try { EnumWindows((h,p)=>{uint owner;GetWindowThreadProcessId(h,out owner);if(owner!=pid)return true;
   var title=new StringBuilder(512);GetWindowText(h,title,512);if(title.Length==0)return true;
   Rect r;GetClientRect(h,out r);var dpi=GetDpiForWindow(h);if(dpi==0)return true;
   var style=GetWindowLongPtr(h,-20).ToInt64();
   rows.Add(new WindowInfo{Title=title.ToString(),Visible=IsWindowVisible(h),Width=(int)Math.Round((r.Right-r.Left)*96.0/dpi),Height=(int)Math.Round((r.Bottom-r.Top)*96.0/dpi),Dpi=dpi,Topmost=(style&8)!=0,ToolWindow=(style&128)!=0});return true;
  },IntPtr.Zero); } finally {SetThreadDpiAwarenessContext(previous);}
  return rows.ToArray();
 }
}
'@
# Reopen after startup has settled; an initial focus-loss event may legitimately hide the popup.
Start-Process -FilePath $exe -ArgumentList '--open-tray' -WindowStyle Hidden
Start-Sleep -Milliseconds 600
$windows = @([CodeBeatTrayProbe]::Inspect($process.Id))
$tray = @($windows | Where-Object { $_.Title -eq 'CodeBeat 托盘' -and $_.Width -gt 0 -and $_.Height -gt 0 })
$result = [pscustomobject]@{pid=$process.Id;exited=$process.HasExited;responding=$process.Responding;stderr_bytes=(Get-Item docs/logs/tray-desktop-stderr.txt).Length;windows=$windows;tray_visible=($tray.Count -eq 1 -and $tray[0].Visible);tray_topmost=($tray.Count -eq 1 -and $tray[0].Topmost);tray_tool_window_style=($tray.Count -eq 1 -and $tray[0].ToolWindow);main_hidden=(@($windows | Where-Object {$_.Title -eq 'CodeBeat' -and $_.Visible}).Count -eq 0)}
$result | ConvertTo-Json -Depth 4 | Set-Content -Encoding UTF8 docs/logs/tray-desktop-startup.json
$result | ConvertTo-Json -Depth 4
if (!$process.HasExited) { Stop-Process -InputObject $process }
if ($result.exited -or !$result.tray_visible -or !$result.tray_topmost -or !$result.main_hidden -or $result.stderr_bytes -ne 0) { throw 'Tray popup startup verification failed; see tray-desktop-startup.json' }
