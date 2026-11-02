#!/bin/sh
# Starts the real app on the desktop of a macOS machine, looks at it and stops it again.
cd "$(dirname "$0")/.."
out=../qa
mkdir -p "$out"
log="$HOME/Library/Application Support/Kumo/kumo.log"
rm -f "$log"

open target/Kumo.app
sleep 15

pid=$(pgrep -x kumo | head -n 1)
echo "process: ${pid:-not running}"
if [ -n "$pid" ]; then
    ps -o pid,rss,%cpu,etime,command -p "$pid"
    footprint "$pid" 2>/dev/null | head -n 12
fi

cat > target/windows.swift <<'SWIFT'
import CoreGraphics
let all = CGWindowListCopyWindowInfo([.optionAll], kCGNullWindowID) as? [[String: Any]] ?? []
let desktop = CGWindowLevelForKey(.desktopWindow), icons = CGWindowLevelForKey(.desktopIconWindow)
print("desktop level \(desktop), icon level \(icons)")
for w in all where (w["kCGWindowOwnerName"] as? String)?.lowercased() == "kumo" {
    print(w["kCGWindowLayer"] ?? "?", w["kCGWindowBounds"] ?? "?", "on screen:", w["kCGWindowIsOnscreen"] ?? "?", "name:", w["kCGWindowName"] ?? "")
}
SWIFT
swift target/windows.swift 2>&1 | head -n 20

screencapture -x "$out/live.png"
sleep 2
screencapture -x "$out/live-later.png"
echo "--- kumo.log"
cat "$log" 2>/dev/null
if [ -n "$pid" ]; then
    kill "$pid"
    sleep 1
    pgrep -x kumo >/dev/null && echo "still running after kill" || echo "stopped"
else
    exit 1
fi
