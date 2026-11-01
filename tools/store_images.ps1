param([string]$Exe = "$env:LOCALAPPDATA\Programs\Kumo\kumo.exe")
# Renders the pictures for the Store listing into package/store: the scene with kumo.exe --shot,
# and the panel on top of it from shots/site-<lang>-<page>.png (see shots/ui-shot.ps1). Needs ffmpeg.
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
$out = Join-Path $root 'package\store'
$tmp = Join-Path $env:TEMP 'kumo-store-images'
New-Item -ItemType Directory -Force $out, $tmp | Out-Null

function Shot($name, $size, $hour) {
  $bmp = Join-Path $tmp "$name.bmp"
  & $Exe --shot $bmp --size $size --warmup 6 --hour $hour --quality=1 | Out-Null
  if (-not (Test-Path $bmp)) { throw "no picture for $name" }
  $bmp
}
function Ff { & ffmpeg -y -hide_banner -loglevel error @args; if ($LASTEXITCODE -ne 0) { throw 'ffmpeg failed' } }

foreach ($s in @(@('evening', '18.6'), @('night', '22'), @('noon', '12'))) {
  Ff -i (Shot $s[0] '1920x1080' $s[1]) (Join-Path $out "screenshot-$($s[0]).png")
}
$back = Shot 'morning' '1920x1080' '7'
foreach ($lang in 'de', 'en') {
  foreach ($page in @(@('2', 'screens'), @('0', 'library'))) {
    $panel = Join-Path $root "shots\site-$lang-$($page[0]).png"
    Ff -i $back -i $panel -filter_complex '[1]crop=1200:837:9:1[p];[0][p]overlay=(W-w)/2:(H-h)/2' (Join-Path $out "screenshot-$($page[1])-$lang.png")
  }
}
$box = Shot 'box' '2160x2160' '18.6'
Ff -i $box (Join-Path $out 'box-2160.png')
Ff -i $box -vf 'scale=1080:1080:flags=lanczos' (Join-Path $out 'box-1080.png')
$poster = Shot 'poster' '1440x2160' '18.6'
Ff -i $poster (Join-Path $out 'poster-1440x2160.png')
Ff -i $poster -vf 'scale=720:1080:flags=lanczos' (Join-Path $out 'poster-720x1080.png')
Ff -i (Join-Path $root 'assets\kumo-icon.png') -vf 'scale=236:236:flags=lanczos,format=rgba,pad=300:300:(ow-iw)/2:(oh-ih)/2:color=black@0' (Join-Path $out 'icon-300.png')
Remove-Item -Recurse -Force $tmp
Get-ChildItem $out | ForEach-Object { '{0,-30}{1,8:N0} KB' -f $_.Name, ($_.Length / 1KB) }
