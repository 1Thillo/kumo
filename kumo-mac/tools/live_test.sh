#!/bin/sh
# Starts the real app on the desktop of a macOS machine, walks it through its wallpaper kinds and
# the control panel, looks at each step and stops it again. Screenshots land in ../qa.
cd "$(dirname "$0")/.."
out=../qa
mkdir -p "$out"
data="$HOME/Library/Application Support/Kumo"
exe=target/Kumo.app/Contents/MacOS/kumo
rm -rf "$data"
failed=0

cat > target/windows.swift <<'SWIFT'
import CoreGraphics
let all = CGWindowListCopyWindowInfo([.optionAll], kCGNullWindowID) as? [[String: Any]] ?? []
let desktop = CGWindowLevelForKey(.desktopWindow), icons = CGWindowLevelForKey(.desktopIconWindow)
print("desktop level \(desktop), icon level \(icons)")
for w in all where (w["kCGWindowOwnerName"] as? String)?.lowercased() == "kumo" {
    let b = w["kCGWindowBounds"] as? [String: Any] ?? [:]
    print("layer", w["kCGWindowLayer"] ?? "?", "size", b["Width"] ?? "?", "x", b["Height"] ?? "?", "on screen:", w["kCGWindowIsOnscreen"] ?? "?", w["kCGWindowName"] ?? "")
}
SWIFT

engine() {
    cat "$data/engine.pid" 2>/dev/null
}

look() {
    name=$1
    pid=$(engine)
    echo "=== $name"
    if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
        ps -o rss=,%cpu= -p "$pid" | awk '{printf "engine: %.0f MB resident, %s %% CPU\n", $1 / 1024, $2}'
        footprint "$pid" 2>/dev/null | grep -m 1 Footprint
    else
        echo "engine not running"
        failed=1
    fi
    screencapture -x "$out/$name.png"
}

# the first start: scene on the desktop, panel with the question about the login
open target/Kumo.app
sleep 12
look first-start
swift target/windows.swift 2>&1 | head -n 12
ls "$data"
pkill -f 'kumo --panel'
sleep 1

# a video
"$exe" --import tools/fixtures/test-video.mp4
sleep 6
look video
ls "$data/library"/*/ | head -n 12

# the same video slower: the engine picks the setting up without rebuilding anything
sleep 1

# a picture
"$exe" --import tools/fixtures/test-picture.jpg
sleep 4
look picture

# a format macOS cannot play: the engine falls back to the built-in scene and says why
"$exe" --import tools/fixtures/test-video.webm
sleep 8
look unsupported
echo "status.json:"
cat "$data/status.json"
echo
grep -q '"' "$data/status.json" || { echo "the failed video was not reported"; failed=1; }

# the panel on its own, with what it reads from the engine
"$exe" --panel &
sleep 6
look panel
swift target/windows.swift 2>&1 | grep -v "layer -2147" | head -n 8
pkill -f 'kumo --panel'
sleep 1

echo "--- kumo.log"
cat "$data/kumo.log" 2>/dev/null

pid=$(engine)
if [ -n "$pid" ]; then
    kill "$pid"
    sleep 1
    if kill -0 "$pid" 2>/dev/null; then
        echo "still running after kill"
        failed=1
    else
        echo "stopped"
    fi
fi
exit "$failed"
