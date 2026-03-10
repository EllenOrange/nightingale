#!/usr/bin/env bash
set -euo pipefail

mkdir -p vendor-bin

OS="$(uname -s)"
ARCH="$(uname -m)"

# ─── ffmpeg ───────────────────────────────────────────────────────────

if [ ! -f vendor-bin/ffmpeg ]; then
  echo "Downloading ffmpeg..."
  case "$OS-$ARCH" in
    Linux-x86_64)
      curl -L "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz" -o /tmp/ffmpeg.tar.xz
      tar -xJf /tmp/ffmpeg.tar.xz -C /tmp --wildcards '*/ffmpeg' --strip-components=1
      mv /tmp/ffmpeg vendor-bin/ffmpeg
      rm /tmp/ffmpeg.tar.xz
      ;;
    Linux-aarch64)
      curl -L "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-arm64-static.tar.xz" -o /tmp/ffmpeg.tar.xz
      tar -xJf /tmp/ffmpeg.tar.xz -C /tmp --wildcards '*/ffmpeg' --strip-components=1
      mv /tmp/ffmpeg vendor-bin/ffmpeg
      rm /tmp/ffmpeg.tar.xz
      ;;
    Darwin-arm64)
      curl -L "https://www.osxexperts.net/ffmpeg7arm.zip" -o /tmp/ffmpeg.zip
      unzip -o /tmp/ffmpeg.zip -d /tmp/ffmpeg_extract
      mv /tmp/ffmpeg_extract/ffmpeg vendor-bin/ffmpeg
      rm -rf /tmp/ffmpeg.zip /tmp/ffmpeg_extract
      ;;
    Darwin-x86_64)
      curl -L "https://www.osxexperts.net/ffmpeg7intel.zip" -o /tmp/ffmpeg.zip
      unzip -o /tmp/ffmpeg.zip -d /tmp/ffmpeg_extract
      mv /tmp/ffmpeg_extract/ffmpeg vendor-bin/ffmpeg
      rm -rf /tmp/ffmpeg.zip /tmp/ffmpeg_extract
      ;;
    *)
      echo "Unsupported platform: $OS-$ARCH"
      exit 1
      ;;
  esac
  chmod +x vendor-bin/ffmpeg
  echo "ffmpeg downloaded"
else
  echo "ffmpeg already present"
fi

# ─── uv ──────────────────────────────────────────────────────────────

if [ ! -f vendor-bin/uv ]; then
  echo "Downloading uv..."
  case "$OS-$ARCH" in
    Linux-x86_64)
      curl -L "https://github.com/astral-sh/uv/releases/latest/download/uv-x86_64-unknown-linux-gnu.tar.gz" -o /tmp/uv.tar.gz
      ;;
    Linux-aarch64)
      curl -L "https://github.com/astral-sh/uv/releases/latest/download/uv-aarch64-unknown-linux-gnu.tar.gz" -o /tmp/uv.tar.gz
      ;;
    Darwin-arm64)
      curl -L "https://github.com/astral-sh/uv/releases/latest/download/uv-aarch64-apple-darwin.tar.gz" -o /tmp/uv.tar.gz
      ;;
    Darwin-x86_64)
      curl -L "https://github.com/astral-sh/uv/releases/latest/download/uv-x86_64-apple-darwin.tar.gz" -o /tmp/uv.tar.gz
      ;;
    *)
      echo "Unsupported platform: $OS-$ARCH"
      exit 1
      ;;
  esac
  mkdir -p /tmp/uvx
  tar -xzf /tmp/uv.tar.gz -C /tmp/uvx
  find /tmp/uvx -name 'uv' -not -name 'uvx' -type f -exec cp {} vendor-bin/uv \;
  rm -rf /tmp/uv.tar.gz /tmp/uvx
  chmod +x vendor-bin/uv
  echo "uv downloaded"
else
  echo "uv already present"
fi

echo "vendor-bin/ ready"
ls -lh vendor-bin/
