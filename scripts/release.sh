#!/usr/bin/env bash
# Publica una versión de Kinora:
#   1. controla que esté todo en orden (main, sin cambios pendientes, la versión en CHANGELOG.md);
#   2. cambia el número de versión, corre los tests y compila;
#   3. arma el instalador de Windows y Kinora.app de la Mac;
#   4. hace el commit "Versión X", la etiqueta vX y los sube;
#   5. crea el Release en GitHub con los dos archivos y las notas del CHANGELOG.
#
#   scripts/release.sh 0.2.0          publica
#   scripts/release.sh 0.2.0 --pre    publica como versión de prueba (pre-release)
set -euo pipefail

VERSION="${1:-}"
PRE=""
[[ "${2:-}" == "--pre" ]] && PRE="--prerelease"
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "usage: scripts/release.sh <version, e.g. 0.2.0> [--pre]" >&2
  exit 1
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
OUT="$ROOT/release/$VERSION"

[[ "$(git branch --show-current)" == "main" ]] || { echo "error: not on main" >&2; exit 1; }
[[ -z "$(git status --porcelain)" ]] || { echo "error: there are uncommitted changes" >&2; exit 1; }
git rev-parse "v$VERSION" >/dev/null 2>&1 && { echo "error: v$VERSION already exists" >&2; exit 1; }
grep -q "^## $VERSION " CHANGELOG.md || { echo "error: CHANGELOG.md has no section '## $VERSION (date)'" >&2; exit 1; }

echo "==> Version $VERSION"
sed -i '' "s/^version = \".*\"/version = \"$VERSION\"/" server/Cargo.toml windows/tray/Cargo.toml
(cd server && cargo metadata --format-version 1 >/dev/null)

echo "==> Tests"
(cd server && cargo test --quiet)

echo "==> Windows installer"
windows/build.sh
(cd windows/tray && cargo metadata --format-version 1 >/dev/null)

echo "==> Mac app"
macos/install.sh --no-install
mkdir -p "$OUT"
cp "windows/build/Kinora-$VERSION-setup.exe" "$OUT/"
ditto -c -k --keepParent macos/build/Kinora.app "$OUT/Kinora-$VERSION-macos.zip"

echo "==> Commit and tag"
git add server/Cargo.toml server/Cargo.lock windows/tray/Cargo.toml windows/tray/Cargo.lock
git commit -q -m "Versión $VERSION"
git tag -a "v$VERSION" -m "Kinora $VERSION"
git push -q origin main "v$VERSION"

echo "==> GitHub release"
# Las notas: la sección de esta versión en CHANGELOG.md, sin su título.
NOTES="$(awk -v v="## $VERSION " 'index($0, v) == 1 { on = 1; next } /^## / { on = 0 } on' CHANGELOG.md)"
gh release create "v$VERSION" $PRE \
  --title "Kinora $VERSION" \
  --notes "$NOTES" \
  "$OUT/Kinora-$VERSION-setup.exe" \
  "$OUT/Kinora-$VERSION-macos.zip"

echo "Released Kinora $VERSION: $(gh release view "v$VERSION" --json url --jq .url)"
