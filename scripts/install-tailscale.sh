#!/bin/sh
# Installs `lifeos web` as a macOS LaunchAgent reachable on the tailnet:
#   https://<machine>.<tailnet>.ts.net:$HTTPS_PORT   the app, via `tailscale serve` (tailnet only)
#   udp <tailscale ip>:$LINK_PORT                     the WebTransport link, straight to lifeos
# The page must come over HTTPS (WebTransport needs a secure context) and
# `tailscale serve` does not carry UDP, hence the two paths.
# Re-run after `make build` to update; it restarts the service.
#
#   HTTPS_PORT  default 8443 (443 is often taken by a local web server)
#   LIFEOS_HOME default ~/lifeos
#   BRIEF_AT    local time the agent writes the morning brief, default 06:30; empty turns it off
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
https_port=${HTTPS_PORT:-8443}
http_port=7420
link_port=7421
home=${LIFEOS_HOME:-$HOME/lifeos}
brief_at=${BRIEF_AT-06:30}
bin=$HOME/.local/bin/lifeos
label=com.lifeos.web
plist=$HOME/Library/LaunchAgents/$label.plist
log=$HOME/Library/Logs/lifeos-web.log

ip=$(tailscale ip -4 | head -n1)
[ -n "$ip" ] || { echo "install-tailscale: tailscale has no IPv4 address; is it up?" >&2; exit 1; }
name=$(tailscale status --json | python3 -c 'import json,sys; print(json.load(sys.stdin)["Self"]["DNSName"].rstrip("."))')

[ -x "$root/target/release/lifeos" ] || { echo "install-tailscale: build first: make build" >&2; exit 1; }
mkdir -p "$(dirname "$bin")" "$(dirname "$plist")" "$(dirname "$log")"
# A new file, not the old one written over: macOS keeps the signature it checked for the old file's inode and kills the
# new binary there (OS_REASON_CODESIGNING).
cp "$root/target/release/lifeos" "$bin.new"
mv -f "$bin.new" "$bin"
[ -d "$home" ] || "$bin" init --home "$home"
package_dir="$home/.lifeos/plugin-packages"
for package in lifeos-github lifeos-car lifeos-paypal; do
  [ -x "$root/target/release/$package" ] || continue
  mkdir -p "$package_dir"
  staged=$(mktemp "$package_dir/.$package.XXXXXX")
  cp -p "$root/target/release/$package" "$staged"
  mv -f "$staged" "$package_dir/$package"
  staged=$(mktemp "$package_dir/.$package.XXXXXX")
  cp "$root/crates/$package/manifest.json" "$staged"
  mv -f "$staged" "$package_dir/$package.manifest.json"
done

cat > "$plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>$label</string>
  <key>ProgramArguments</key>
  <array>
    <string>$bin</string><string>web</string>
    <string>--host</string><string>127.0.0.1</string>
    <string>--port</string><string>$http_port</string>
    <string>--link-host</string><string>$ip</string>
    <string>--link-port</string><string>$link_port</string>
  </array>
  <key>EnvironmentVariables</key>
  <dict>
    <key>LIFEOS_HOME</key><string>$home</string>$([ -n "$brief_at" ] && printf '\n    <key>LIFEOS_BRIEF_AT</key><string>%s</string>' "$brief_at")
    <key>PATH</key><string>$(dirname "$bin"):/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin</string>
  </dict>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>ThrottleInterval</key><integer>10</integer>
  <key>StandardOutPath</key><string>$log</string>
  <key>StandardErrorPath</key><string>$log</string>
</dict>
</plist>
EOF
plutil -lint "$plist" >/dev/null

launchctl bootout "gui/$(id -u)/$label" 2>/dev/null || true
# bootout returns before the old service is gone; bootstrap fails until it is.
i=0
while launchctl print "gui/$(id -u)/$label" >/dev/null 2>&1 && [ $i -lt 50 ]; do sleep 0.2; i=$((i + 1)); done
launchctl bootstrap "gui/$(id -u)" "$plist"
tailscale serve --bg --https="$https_port" "http://127.0.0.1:$http_port" >/dev/null

echo "lifeos: https://$name:$https_port (link: udp $ip:$link_port, log: $log)"
