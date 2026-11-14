$res = @{ ok = 0; real = ''; err = '' }
try {
  $cur = [GatitoNS.GatitoAudio]::DefaultId(1)
  $cn = EpName 'Capture' $cur
  if ($cn -and ($cn -notmatch 'CABLE|VB-Audio|VB-Output')) { $res.real = $cn }
  $cap = FindEp 'Capture' 'CABLE Output|\(VB-Output\)'
  if ($cap) { if ($cap -ne $cur) { $null = [GatitoNS.GatitoAudio]::SetDefault($cap) }; $res.ok = 1 }
} catch { $res.err = $_.Exception.Message }
Write-Output (ConvertTo-Json -Compress -InputObject $res)
