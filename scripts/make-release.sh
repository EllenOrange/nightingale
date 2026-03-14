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

echo "==> Packaging $ARCHIVE..."
tar czf "$ARCHIVE" -C "target/$TARGET/release" nightingale

SIZE=$(du -h "$BINARY" | cut -f1)
ARCHIVE_SIZE=$(du -h "$ARCHIVE" | cut -f1)

echo ""
echo "Done!"
echo "  Binary:  $BINARY ($SIZE)"
echo "  Archive: $ARCHIVE ($ARCHIVE_SIZE)"
