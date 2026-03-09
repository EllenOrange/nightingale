#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VENV_DIR="$SCRIPT_DIR/.venv"

# PyTorch/transformers require Python >=3.10, <3.14
find_python() {
    for candidate in python3.13 python3.12 python3.11 python3.10 python3 python; do
        local bin
        bin="$(command -v "$candidate" 2>/dev/null)" || continue
        if "$bin" -c "import sys; exit(0 if (3,10) <= sys.version_info[:2] <= (3,13) else 1)" 2>/dev/null; then
            echo "$bin"
            return
        fi
    done
    echo "[nightingale] WARNING: No Python 3.10-3.13 found, falling back to python3" >&2
    echo "python3"
}

if [ ! -d "$VENV_DIR" ]; then
    PYTHON_BIN="$(find_python)"
    PY_VER=$("$PYTHON_BIN" -c 'import sys; print(f"{sys.version_info.major}.{sys.version_info.minor}")')
    echo "[nightingale] Creating Python virtual environment with $PYTHON_BIN (Python $PY_VER)..."
    "$PYTHON_BIN" -m venv "$VENV_DIR"
fi

echo "[nightingale] Activating virtual environment..."
source "$VENV_DIR/bin/activate"

echo "[nightingale] Installing dependencies..."
pip install --upgrade pip
pip install -r "$SCRIPT_DIR/requirements.txt"

echo "[nightingale] Setup complete. Virtual environment at: $VENV_DIR"
echo "[nightingale] Run analyzer with: $VENV_DIR/bin/python $SCRIPT_DIR/analyze.py <audio_path> <output_dir>"
