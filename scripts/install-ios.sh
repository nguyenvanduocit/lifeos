#!/bin/sh
# Builds the iPhone app (apps/ios) and installs it on the iPhone plugged into this Mac.
# Needs Xcode with your Apple ID added (Xcode → Settings → Accounts) and the phone in Developer Mode.
# Team: TEAM=XXXXXXXXXX, or the first team in Xcode's accounts.
set -eu
cd "$(dirname "$0")/../apps/ios"
export DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}"

team="${TEAM:-}"
if [ -z "$team" ]; then
  team=$(defaults read com.apple.dt.Xcode IDEProvisioningTeamByIdentifier 2>/dev/null \
    | grep -o 'teamID = [A-Z0-9]*' | head -1 | awk '{print $3}')
fi
[ -n "$team" ] || { echo "no team: add your Apple ID in Xcode → Settings → Accounts, or set TEAM=" >&2; exit 1; }

device=$(xcrun devicectl list devices --hide-default-columns --columns Identifier --hide-headers 2>/dev/null \
  | awk 'NF' | head -1)
[ -n "$device" ] || { echo "no iPhone: plug it in, unlock it and trust this Mac" >&2; exit 1; }

xcodegen generate --quiet
xcodebuild -project LifeOS.xcodeproj -scheme LifeOS -configuration Release \
  -destination "id=$device" -derivedDataPath build -allowProvisioningUpdates \
  DEVELOPMENT_TEAM="$team" -quiet build
app=build/Build/Products/Release-iphoneos/LifeOS.app
xcrun devicectl device install app --device "$device" "$app"
xcrun devicectl device process launch --device "$device" dev.aiocean.lifeos || \
  echo "installed; if it will not open, trust the developer in Settings → General → VPN & Device Management"
