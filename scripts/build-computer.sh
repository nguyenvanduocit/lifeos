#!/bin/sh
# Builds LifeOS Computer.app (crates/lifeos-computer): the menu bar app that lets lifeos's agents see and work a Mac's
# screen. With --install it also puts the app in /Applications and starts it.
#
#   scripts/build-computer.sh              target/release/LifeOS Computer.app and a zip beside it, for other Macs
#   scripts/build-computer.sh --install    and /Applications/LifeOS Computer.app, (re)started
#
# macOS keeps the Accessibility and Screen Recording grants for an app by its signature. An ad-hoc signature changes
# with every build, so every update would ask again: the app is signed with a certificate of its own, made once in
# the login keychain ("LifeOS Computer Signing", self-signed, for code signing only), so the grants outlive updates.
# On another Mac, copy the zip, unzip it into /Applications and open it once with right-click → Open.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
identity="LifeOS Computer Signing"
app="$root/target/release/LifeOS Computer.app"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -n1)

cargo build --release -p lifeos-computer --manifest-path "$root/Cargo.toml"

if ! security find-certificate -c "$identity" >/dev/null 2>&1; then
  echo "build-computer: making the signing certificate \"$identity\" in the login keychain"
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT
  openssl req -x509 -newkey rsa:2048 -nodes -days 3650 -subj "/CN=$identity" \
    -addext "keyUsage=critical,digitalSignature" -addext "extendedKeyUsage=critical,codeSigning" \
    -addext "basicConstraints=critical,CA:false" \
    -keyout "$tmp/key.pem" -out "$tmp/cert.pem" 2>/dev/null
  # The keychain reads only the old PKCS#12 ciphers.
  openssl pkcs12 -export -legacy -inkey "$tmp/key.pem" -in "$tmp/cert.pem" -out "$tmp/id.p12" -passout pass:lifeos \
    -name "$identity" 2>/dev/null ||
    openssl pkcs12 -export -inkey "$tmp/key.pem" -in "$tmp/cert.pem" -out "$tmp/id.p12" -passout pass:lifeos \
      -name "$identity" -keypbe PBE-SHA1-3DES -certpbe PBE-SHA1-3DES -macalg sha1
  security import "$tmp/id.p12" -k "$HOME/Library/Keychains/login.keychain-db" -P lifeos -T /usr/bin/codesign >/dev/null
fi

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$root/target/release/lifeos-computer" "$app/Contents/MacOS/lifeos-computer"
cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key><string>dev.aiocean.lifeos.computer</string>
  <key>CFBundleName</key><string>LifeOS Computer</string>
  <key>CFBundleDisplayName</key><string>LifeOS Computer</string>
  <key>CFBundleExecutable</key><string>lifeos-computer</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
EOF
plutil -lint "$app/Contents/Info.plist" >/dev/null
codesign --force --timestamp=none --options runtime --sign "$identity" "$app"
codesign --verify --strict "$app"
(cd "$(dirname "$app")" && rm -f "LifeOS Computer.zip" && ditto -c -k --keepParent "LifeOS Computer.app" "LifeOS Computer.zip")
echo "build-computer: $app"

if [ "${1:-}" = "--install" ]; then
  dest="/Applications/LifeOS Computer.app"
  pkill -x lifeos-computer 2>/dev/null || true
  i=0
  while pgrep -x lifeos-computer >/dev/null && [ $i -lt 50 ]; do sleep 0.1; i=$((i + 1)); done
  rm -rf "$dest.new"
  ditto "$app" "$dest.new"
  rm -rf "$dest"
  mv "$dest.new" "$dest"
  open "$dest"
  echo "build-computer: installed $dest and started it"
fi
