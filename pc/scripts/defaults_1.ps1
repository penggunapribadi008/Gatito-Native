$rid = '@@RID@@'
$rname = '@@RNAME@@'
$res = @{ cap = -1; ren = -1; err = '' }
try {
  for ($i = 0; $i -lt 1; $i++) {
    $cap = FindEp 'Capture' 'CABLE Output|\(VB-Output\)'
    if ($cap) { $res.cap = [GatitoNS.GatitoAudio]::SetDefault($cap) }
    if ($rid -and ($rname -notmatch 'CABLE|VB-Audio|VB-Output')) {
      $cur = [GatitoNS.GatitoAudio]::DefaultId(0)
      if ($cur -ne $rid) { $res.ren = [GatitoNS.GatitoAudio]::SetDefault($rid) } else { $res.ren = 0 }
    }
    if ($i -lt 0) { Start-Sleep -Seconds 3 }
  }
} catch { $res.err = $_.Exception.Message }
Write-Output (ConvertTo-Json -Compress -InputObject $res)
