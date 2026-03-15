#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR/.."

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS-$ARCH" in
  Linux-x86_64)   TARGET="x86_64-unknown-linux-gnu" ;;
  Linux-aarch64)  TARGET="aarch64-unknown-linux-gnu" ;;
  Darwin-arm64)   TARGET="aarch64-apple-darwin" ;;
  Darwin-x86_64)  TARGET="x86_64-apple-darwin" ;;
  *)
    echo "Unsupported platform: $OS-$ARCH"
    exit 1
    ;;
esac

echo "==> Platform: $TARGET"

echo "==> Building release binary..."
if [ -f .env ]; then
  set -a; source .env; set +a
fi
cargo build --release --target "$TARGET"

BINARY="target/$TARGET/release/nightingale"
ARCHIVE="nightingale-$TARGET.tar.gz"
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')

echo "==> Packaging $ARCHIVE..."

STAGING=$(mktemp -d)
trap 'rm -rf "$STAGING"' EXIT

if [ "$OS" = "Darwin" ]; then
  APP="$STAGING/Nightingale.app"
  mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

  cp "$BINARY" "$APP/Contents/MacOS/nightingale"

  sed "s/__VERSION__/$VERSION/g" packaging/macos/Info.plist > "$APP/Contents/Info.plist"

  ICONSET="$STAGING/nightingale.iconset"
  mkdir -p "$ICONSET"
  for SIZE_PX in 16 32 128 256 512; do
    sips -z "$SIZE_PX" "$SIZE_PX" assets/images/logo_square.png --out "$ICONSET/icon_${SIZE_PX}x${SIZE_PX}.png" >/dev/null
    DOUBLE=$((SIZE_PX * 2))
    sips -z "$DOUBLE" "$DOUBLE" assets/images/logo_square.png --out "$ICONSET/icon_${SIZE_PX}x${SIZE_PX}@2x.png" >/dev/null
  done
  iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/nightingale.icns"

  tar czf "$ARCHIVE" -C "$STAGING" Nightingale.app

elif [ "$OS" = "Linux" ]; then
  PKGDIR="$STAGING/nightingale"
  mkdir -p "$PKGDIR"
  cp "$BINARY" "$PKGDIR/nightingale"
  cp packaging/linux/nightingale.desktop "$PKGDIR/"
  cp assets/images/logo_square.png "$PKGDIR/nightingale.png"

  tar czf "$ARCHIVE" -C "$STAGING" nightingale
fi

SIZE=$(du -h "$BINARY" | cut -f1)
ARCHIVE_SIZE=$(du -h "$ARCHIVE" | cut -f1)

echo ""
echo "Done!"
echo "  Binary:  $BINARY ($SIZE)"
echo "  Archive: $ARCHIVE ($ARCHIVE_SIZE)"
