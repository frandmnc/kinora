#!/usr/bin/env bash
# Arma el instalador de Kinora para Windows desde la Mac: Kinora-<versión>-setup.exe, en
# windows/build/. Compila la web, el servidor y la app de la bandeja para Windows
# (x86_64-pc-windows-gnu) y los junta con NSIS.
#
# Hace falta, una vez:
#   brew install rustup mingw-w64 makensis
#   $(brew --prefix rustup)/bin/rustup toolchain install <la versión de rustc> --target x86_64-pc-windows-gnu
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUILD="$ROOT/windows/build"
TARGET=x86_64-pc-windows-gnu
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/server/Cargo.toml" | head -1)"

# El Rust de Homebrew no trae Windows: se compila con el de rustup, de la misma versión.
RUSTUP_BIN="$(brew --prefix rustup)/bin"
TOOLCHAIN="$(rustc --version | cut -d' ' -f2)"
if ! "$RUSTUP_BIN/rustup" target list --installed --toolchain "$TOOLCHAIN" 2>/dev/null | grep -q "$TARGET"; then
  echo "error: missing the Windows target. Run: $RUSTUP_BIN/rustup toolchain install $TOOLCHAIN --target $TARGET" >&2
  exit 1
fi
cargo_windows() {
  PATH="$RUSTUP_BIN:$PATH" cargo "+$TOOLCHAIN" build --release --target "$TARGET" "$@"
}

echo "==> Building the web app (Vite)"
cd "$ROOT/web"
[[ -d node_modules ]] || npm ci
npm run build

echo "==> Building the server for Windows"
cd "$ROOT/server"
cargo_windows

echo "==> Building the tray app for Windows"
cd "$ROOT/windows/tray"
# La app de la bandeja lleva la misma versión que el servidor.
sed -i '' "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml
cargo_windows

echo "==> Packing the installer (NSIS)"
mkdir -p "$BUILD"
cd "$ROOT/windows"
# makensis lee el script en UTF-8 solo si el entorno lo está (en la Mac, LANG suele venir vacío).
LC_ALL=en_US.UTF-8 makensis -V2 \
  "-DVERSION=$VERSION" \
  "-DSERVER=$ROOT/server/target/$TARGET/release/kinora.exe" \
  "-DTRAY=$ROOT/windows/tray/target/$TARGET/release/Kinora.exe" \
  "-DOUTFILE=$BUILD/Kinora-$VERSION-setup.exe" \
  installer.nsi

echo "Built $BUILD/Kinora-$VERSION-setup.exe"
