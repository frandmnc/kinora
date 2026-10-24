#!/usr/bin/env bash
# Arma Kinora.app (la app de la barra de menú con el servidor adentro, y la web dentro del
# servidor) y la instala en /Applications. Para actualizar, se corre de nuevo.
#
#   macos/install.sh               arma, instala y abre
#   macos/install.sh --no-install  solo arma, en macos/build/Kinora.app
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUILD="$ROOT/macos/build"
APP="$BUILD/Kinora.app"
INSTALL=1
if [[ "${1:-}" == "--no-install" ]]; then
  INSTALL=0
fi

command -v ffmpeg >/dev/null || echo "warning: ffmpeg not found in PATH (brew install ffmpeg): imports and conversions need it"

echo "==> Building the web app (Vite)"
cd "$ROOT/web"
[[ -d node_modules ]] || npm ci
npm run build

echo "==> Building the server (cargo build --release)"
cd "$ROOT/server"
cargo build --release

echo "==> Building the menu bar app (swiftc)"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
swiftc -O -parse-as-library -swift-version 5 \
  -target "$(uname -m)-apple-macos13.0" \
  -o "$APP/Contents/MacOS/Kinora" \
  "$ROOT"/macos/Kinora/*.swift
cp "$ROOT/server/target/release/kinora" "$APP/Contents/MacOS/kinora-server"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/server/Cargo.toml" | head -1)"
sed "s/__VERSION__/$VERSION/g" "$ROOT/macos/Info.plist" > "$APP/Contents/Info.plist"

echo "==> Drawing the icon"
rm -rf "$BUILD/Kinora.iconset"
swift "$ROOT/macos/make-icon.swift" "$BUILD/Kinora.iconset"
iconutil -c icns -o "$APP/Contents/Resources/Kinora.icns" "$BUILD/Kinora.iconset"

# Firma local (sin cuenta de desarrollador): alcanza para correr en esta compu.
codesign --force --sign - "$APP/Contents/MacOS/kinora-server"
codesign --force --sign - "$APP"

if [[ $INSTALL == 0 ]]; then
  echo "Built $APP (version $VERSION)"
  exit 0
fi

echo "==> Installing in /Applications"
if pgrep -x Kinora >/dev/null; then
  # Al salir, la app apaga el servidor (y el ffmpeg de la cola, si hay uno).
  osascript -e 'quit app "Kinora"' || true
  for _ in $(seq 1 50); do
    pgrep -x Kinora >/dev/null || break
    sleep 0.2
  done
fi
rm -rf /Applications/Kinora.app
cp -R "$APP" /Applications/
open /Applications/Kinora.app
echo "Kinora $VERSION installed in /Applications and running: look for the icon in the menu bar"
