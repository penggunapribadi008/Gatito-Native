$code = 0
try {
  [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
  $ProgressPreference = 'SilentlyContinue'
  $dir = ''
  if ($install) {
    $dir = Join-Path $env:SystemRoot ('Temp\GatitoVB_' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $dir -Force | Out-Null
    $zip = Join-Path $dir 'vb.zip'
    $ok = $false
    foreach ($u in @('https://download.vb-audio.com/Download_CABLE/VBCABLE_Driver_Pack45.zip', 'https://download.vb-audio.com/Download_CABLE/VBCABLE_Driver_Pack43.zip')) {
      try {
        Invoke-WebRequest -UseBasicParsing -Uri $u -OutFile $zip -TimeoutSec 120
        if ((Get-Item $zip).Length -gt 100000) { $ok = $true; break }
      } catch {}
    }
    if (-not $ok) {
      $code = $code -bor 4
    } else {
      $pkg = Join-Path $dir 'pkg'
      Expand-Archive -Path $zip -DestinationPath $pkg -Force
      $all = @(Get-ChildItem -Path $pkg -Recurse -Filter 'VBCABLE_Setup*.exe')
      $arch = $env:PROCESSOR_ARCHITECTURE
      $setup = $null
      if ($arch -eq 'ARM64') { $setup = $all | Where-Object { $_.Name -match 'arm' } | Select-Object -First 1 } elseif ($arch -eq 'AMD64') { $setup = $all | Where-Object { $_.Name -match 'x64' } | Select-Object -First 1 }
      if (-not $setup) { $setup = $all | Where-Object { $_.Name -notmatch 'x64|arm' } | Select-Object -First 1 }
      if (-not $setup) {
        $code = $code -bor 8
      } else {
        Start-Process -FilePath $setup.FullName -ArgumentList '-i', '-h' -WorkingDirectory $setup.DirectoryName -Wait | Out-Null
      }
    }
  }
  $id = ''
  if ($install) { $tries = 60 } else { $tries = 3 }
  for ($i = 0; $i -lt $tries; $i++) {
    $id = FindEp 'Capture' 'CABLE Output|\(VB-Output\)'
    if ($id) { break }
    Start-Sleep -Seconds 1
  }
  if (-not $id) {
    $code = $code -bor 16
  } else {
    $code = $code -bor 1
    try { if ([GatitoNS.GatitoAudio]::Rename($id, $name) -eq 0) { $code = $code -bor 2 } } catch {}
  }
} catch {
  $code = $code -bor 32
} finally {
  if ($dir) { Remove-Item -Path $dir -Recurse -Force -ErrorAction SilentlyContinue }
}
