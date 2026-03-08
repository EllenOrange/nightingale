#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VENV_DIR="$SCRIPT_DIR/.venv"

if [ ! -d "$VENV_DIR" ]; then
    echo "[nightingale] Creating Python virtual environment..."
    python3 -m venv "$VENV_DIR"
fi

echo "[nightingale] Activating virtual environment..."
source "$VENV_DIR/bin/activate"

echo "[nightingale] Installing dependencies..."
pip install --upgrade pip
pip install -r "$SCRIPT_DIR/requirements.txt"

echo "[nightingale] Setup complete. Virtual environment at: $VENV_DIR"
echo "[nightingale] Run analyzer with: $VENV_DIR/bin/python $SCRIPT_DIR/analyze.py <audio_path> <output_dir>"
