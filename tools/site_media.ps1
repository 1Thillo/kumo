param([string]$Exe = "$env:LOCALAPPDATA\Programs\Kumo\kumo.exe", [switch]$StillsOnly)
# Renders site/media from the real scene with kumo.exe --shot: the stills for the day slider, the
# frames of the scrolling day and the hero loop. Runs offscreen, no window opens. Needs ffmpeg.
# The camera sways on a sine and the hero fades its end into its start, so it loops cleanly.
$ErrorActionPreference = 'Stop'
$out = Join-Path (Split-Path $PSScriptRoot) 'site\media'
$tmp = Join-Path $env:TEMP 'kumo-site-frames'
New-Item -ItemType Directory -Force $out | Out-Null
$c = [Globalization.CultureInfo]::InvariantCulture
$Fps = 30
$Fade = 45                    # frames
$Orbit = 0.028                # camera.orbitSpeed of the pagoda scene
$Start = 25.4                 # seconds of simulation before the first frame

function Shot($file, $size, $step, $hour, $angle) {
  $t = ($Start * $Fps + $step + 0.5) / $Fps
  $rotate = ($angle / ($t * $Orbit)).ToString('F7', $c)
  & $Exe --shot $file --size $size --warmup $t.ToString('F4', $c) --hour $hour.ToString('F3', $c) --quality=1 "--rotate=$rotate" | Out-Null
  if ($LASTEXITCODE -ne 0) { throw "render failed at step $step" }
}

function Clip($name, $seconds, $hourOf, $sway, $crf, $av1) {
  $n = $seconds * $Fps
  Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
  New-Item -ItemType Directory -Force $tmp | Out-Null
  for ($k = 0; $k -lt $n + $Fade; $k++) {
    $hour = & $hourOf (($k % $n) / $n)
    $angle = $sway * [math]::Sin(2 * [math]::PI * $k / $n)
    Shot ('{0}\{1:D4}.bmp' -f $tmp, $k) '2000x900' $k $hour $angle
  }
  $end = $n + $Fade
  $mix = "[0:v]crop=1600:900:0:0,split=3[a][b][c];" +
    "[a]trim=start_frame=${n}:end_frame=$end,setpts=PTS-STARTPTS[tail];" +
    "[b]trim=end_frame=$Fade,setpts=PTS-STARTPTS[head];" +
    "[c]trim=start_frame=${Fade}:end_frame=$n,setpts=PTS-STARTPTS[rest];" +
    "[tail][head]xfade=transition=fade:duration=$($Fade / $Fps):offset=0[mix];" +
    "[mix][rest]concat=n=2:v=1:a=0,format=yuv420p[v]"
  $in = @('-y', '-hide_banner', '-loglevel', 'error', '-framerate', $Fps, '-i', "$tmp\%04d.bmp", '-filter_complex', $mix, '-map', '[v]', '-an')
  & ffmpeg @in -c:v libx264 -preset slow -crf $crf -profile:v high -movflags +faststart (Join-Path $out "$name.mp4")
  & ffmpeg @in -c:v libsvtav1 -preset 5 -crf $av1 (Join-Path $out "$name.webm")
  Remove-Item -Recurse -Force $tmp
}

# the stills share one moment and one camera, so the day slider only changes the light
foreach ($h in @(@('morning', 7.0), @('noon', 12.0), @('evening', 18.6), @('night', 22.0))) {
  $bmp = Join-Path $env:TEMP "kumo-$($h[0]).bmp"
  Shot $bmp '1920x1080' 0 $h[1] 0
  & ffmpeg -y -hide_banner -loglevel error -i $bmp -q:v 80 (Join-Path $out "$($h[0]).webp")
  if ($h[0] -eq 'evening') {
    & ffmpeg -y -hide_banner -loglevel error -i $bmp -q:v 3 (Join-Path $out 'poster.jpg')
    & ffmpeg -y -hide_banner -loglevel error -i $bmp -vf 'crop=1920:1008:0:36,scale=1200:630' -q:v 4 (Join-Path $out 'og.jpg')
  }
  Remove-Item $bmp
}

# the scrolling day: closer together around sunset, where the light changes fastest
$hours = 5.5, 7, 9, 12, 15, 17, 18, 18.6, 19.2, 20, 22, 24   # keep in step with DAY_HOURS in main.js
for ($i = 0; $i -lt $hours.Count; $i++) {
  $bmp = Join-Path $env:TEMP 'kumo-day.bmp'
  Shot $bmp '1600x900' 0 ($hours[$i] % 24.0) 0
  & ffmpeg -y -hide_banner -loglevel error -i $bmp -q:v 74 (Join-Path $out ('day-{0:D2}.webp' -f $i))
  Remove-Item $bmp
}

if (-not $StillsOnly) { Clip 'hero' 14 { param($p) 18.6 } 0.11 27 40 }

Get-ChildItem $out | ForEach-Object { '{0,-16}{1,8:N0} KB' -f $_.Name, ($_.Length / 1KB) }
