$res = @{ ok = 0; skip = 0; err = '' }
try {
  $cur = [GatitoNS.GatitoAudio]::DefaultId(1)
  $cn = EpName 'Capture' $cur
  if ($cn -notmatch 'CABLE|VB-Audio|VB-Output') { $res.skip = 1 } else {
    $id = FindEp 'Capture' ('^' + [regex]::Escape('@@LABEL@@') + '$')
    if (-not $id) { $res.err = 'notfound' } else { $res.ok = 1; $null = [GatitoNS.GatitoAudio]::SetDefault($id) }
  }
} catch { $res.err = $_.Exception.Message }
Write-Output (ConvertTo-Json -Compress -InputObject $res)
