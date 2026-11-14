$r = [GatitoNS.GatitoAudio]::DefaultId(0)
$c = [GatitoNS.GatitoAudio]::DefaultId(1)
$o = @{ rid = $r; rname = (EpName 'Render' $r); cid = $c; cname = (EpName 'Capture' $c) }
Write-Output (ConvertTo-Json -Compress -InputObject $o)
