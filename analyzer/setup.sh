#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VENV_DIR="$SCRIPT_DIR/.venv"

# transformers>=5.2.0 (Voxtral) requires Python >=3.10
find_python() {
    for candidate in python3 python3.13 python3.12 python3.11 python3.10; do
        if command -v "$candidate" &>/dev/null; then
            if "$candidate" -c "import sys; exit(0 if sys.version_info >= (3,10) else 1)" 2>/dev/null; then
                echo "$candidate"
                return
            fi
        fi
    done
    echo "[nightingale] WARNING: No Python >=3.10 found, falling back to python3" >&2
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
