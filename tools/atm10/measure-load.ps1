# measure-load.ps1 [-Minutes 20] [-FloorMB 700] [-HoldSeconds 60] [-LineSeconds 10] [-Profile DIR] [game arguments for play-atm10.ts...]
# Starts the game with play-atm10.ts (every carry), samples the client's memory every 0.7 s, and stops it once it
# has joined a world and run -HoldSeconds more, after -Minutes, or when the machine's free memory falls under -FloorMB
# (so the PC never runs out). Prints the log's milestones and the memory peaks; the log stays in
# ..\play\game\logs\latest.log, and a line every -LineSeconds goes to memory.csv beside it (in the profile).
param([int]$Minutes = 20, [int]$FloorMB = 700, [int]$HoldSeconds = 60, [int]$LineSeconds = 10, [string]$Profile = '', [Parameter(ValueFromRemainingArguments = $true)][string[]]$GameArgs)
$here = if ($env:ATM10_WORK) { $env:ATM10_WORK } else { Split-Path -Parent $PSScriptRoot }
# BUN: bun.exe (default: the one on PATH). ATM10_WORK and HORIZON_REPO go through to play-atm10.ts.
$bun = if ($env:BUN) { $env:BUN } else { (Get-Command bun).Source }
# -Profile: the launcher's --profile (default ..\play); the log and the launcher's output are kept there.
$prof = if ($Profile) { $Profile } else { Join-Path $here 'play' }
if ($Profile) { $GameArgs = @('--profile', $Profile) + $GameArgs }
$log = Join-Path $prof 'game\logs\latest.log'
$out = Join-Path $prof 'measure-stdout.log'
New-Item -ItemType Directory -Force $prof | Out-Null
$argList = @("`"$PSScriptRoot\play-atm10.ts`"") + ($GameArgs | ForEach-Object { "`"$_`"" })
$start = Get-Date
$launcher = Start-Process -FilePath $bun -ArgumentList $argList -RedirectStandardOutput $out `
  -RedirectStandardError "$out.err" -PassThru -WindowStyle Hidden
$csv = Join-Path $prof 'memory.csv'
$drive = (Resolve-Path $prof).Drive.Name
Set-Content -Path $csv -Value 'seconds,working_set_mb,private_mb,free_mb'
$nextLine = 0
$peakWs = 0; $peakPrivate = 0; $minFree = [int]::MaxValue; $client = $null; $joined = $null; $why = 'timeout'
while (((Get-Date) - $start).TotalMinutes -lt $Minutes) {
  Start-Sleep -Milliseconds 700
  if (-not $client) {
    # The launcher's own child: other games may run beside it.
    $c = Get-CimInstance Win32_Process -Filter "Name='horizon-client.exe' AND ParentProcessId=$($launcher.Id)" |
      Select-Object -First 1
    if ($c) { $client = Get-Process -Id $c.ProcessId -ErrorAction SilentlyContinue }
  }
  $free = [int]((Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory / 1KB)
  if ($free -lt $minFree) { $minFree = $free }
  if ($client) {
    $client.Refresh()
    if ($client.HasExited) { $why = "the game exited with code $($client.ExitCode)"; break }
    if ($client.WorkingSet64 -gt $peakWs) { $peakWs = $client.WorkingSet64 }
    if ($client.PrivateMemorySize64 -gt $peakPrivate) { $peakPrivate = $client.PrivateMemorySize64 }
    # A line every -LineSeconds: seconds since start, the game's working set and private bytes, the machine's free MB.
    $t = [int]((Get-Date) - $start).TotalSeconds
    if ($t -ge $nextLine) {
      Add-Content -Path $csv -Value ("{0},{1},{2},{3}" -f $t, [int]($client.WorkingSet64 / 1MB), [int]($client.PrivateMemorySize64 / 1MB), $free)
      $nextLine = $t + $LineSeconds
    }
  } elseif ($launcher.HasExited) { $why = 'the launcher exited before the game started'; break }
  if ($free -lt $FloorMB) { $why = "free memory fell to $free MB"; break }
  $disk = [int]((Get-PSDrive $drive).Free / 1GB)
  if ($disk -lt 3) { $why = "drive ${drive}: fell to $disk GB free"; break }
  # The log of an earlier start says "joined" too: only this start's log (written after it began) counts.
  if (-not $joined -and $client -and (Test-Path $log) -and (Get-Item $log).LastWriteTime -gt $start -and
      (Select-String -Path $log -Pattern 'joined the game' -Quiet)) { $joined = Get-Date }
  if ($joined -and ((Get-Date) - $joined).TotalSeconds -ge $HoldSeconds) { $why = "joined a world and ran $HoldSeconds s"; break }
}
$wall = [int]((Get-Date) - $start).TotalSeconds
if ($client -and -not $client.HasExited) { "peak working set reported by Windows: {0} MB" -f [int]($client.PeakWorkingSet64 / 1MB) }
if ($client -and -not $client.HasExited) { Stop-Process -Id $client.Id -Force }
if (-not $launcher.HasExited) { Stop-Process -Id $launcher.Id -Force }
"stopped: $why after $wall s"
"sampled peaks: working set {0} MB, private {1} MB; machine's lowest free memory {2} MB" -f [int]($peakWs / 1MB), [int]($peakPrivate / 1MB), $minFree
if (Test-Path $log) {
  Select-String -Path $log -Pattern 'read the content|content root|content from|content not loaded|not loaded:|load core tables|load pack tables|UI bundle ready|core-ready|load items|resources loaded in|rest-ready|world data preloaded|joined the game|panicked|ERROR' |
    Select-Object -First 40 | ForEach-Object { $_.Line.Substring(0, [Math]::Min(220, $_.Line.Length)) }
  "warnings in the log: {0}" -f (Select-String -Path $log -Pattern '/WARN\]' | Measure-Object).Count
}
