$res = @{ ok = 0; err = '' }
try {
  $id = FindEp 'Capture' ('^' + [regex]::Escape('@@LABEL@@') + '$')
  if (-not $id) { $res.err = 'notfound' } else { $res.ok = 1; $null = [GatitoNS.GatitoAudio]::SetDefault($id) }
} catch { $res.err = $_.Exception.Message }
Write-Output (ConvertTo-Json -Compress -InputObject $res)
