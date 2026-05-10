#!/usr/bin/env python3
"""Persistent analyzer server for Nightingale.

Binds a TCP loopback socket and exchanges newline-delimited JSON with the
Rust client. Stdout/stderr are reserved for plain logs aside from one
handshake line emitted on startup.

Handshake (stdout, single line):
  {"event":"ready","port":N,"token":"...","device":"..."}

Wire protocol (NDJSON over TCP, one JSON object per line):
  Client -> server:
    {"type":"hello","token":"..."}
    {"type":"analyze","hash":"...","audio_path":"...","cache_path":"...", ...}
    {"type":"quit"}
  Server -> client:
    {"type":"hello_ack"}
    {"type":"progress","pct":N,"msg":"..."}
    {"type":"done","hash":"..."}
    {"type":"error","kind":"oom"|"generic","msg":"..."}
"""

import json
import os
import secrets
import socket
import sys

if os.name == "nt":
    import huggingface_hub.file_download as _hf_dl
    _hf_dl.are_symlinks_supported = lambda *_a, **_kw: False

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from whisper_compat import detect_device, compute_type_for, is_oom, free_gpu, set_progress_sink
from pipeline import run_pipeline

_whisper_model = None
_whisper_key = None  # (model_name, device, compute_type)


def _clear_models():
    global _whisper_model, _whisper_key
    if _whisper_model is not None:
        del _whisper_model
    _whisper_model = None
    _whisper_key = None
    free_gpu()


def _get_whisper(model_name, device, compute_type):
    global _whisper_model, _whisper_key
    key = (model_name, device, compute_type)
    if _whisper_model is not None and _whisper_key == key:
        return _whisper_model
    if _whisper_model is not None:
        del _whisper_model
        _whisper_model = None
        free_gpu()
    import whisperx
    _whisper_model = whisperx.load_model(
        model_name, device, compute_type=compute_type, task="transcribe",
    )
    _whisper_key = key
    return _whisper_model


def process_song(cmd, device):
    audio_path = os.path.abspath(cmd["audio_path"])
    output_dir = os.path.abspath(cmd["cache_path"])
    file_hash = cmd["hash"]
    model_name = cmd.get("model", "large-v3")
    beam_size = cmd.get("beam_size", 8)
    batch_size = cmd.get("batch_size", 8)
    separator = cmd.get("separator", "karaoke")
    lyrics_path = cmd.get("lyrics")
    language_override = cmd.get("language")

    c_type = compute_type_for(device)
    actual_device = "cpu" if device == "mps" else device

    run_pipeline(
        audio_path, output_dir, file_hash, device,
        model_name=model_name,
        beam_size=beam_size,
        batch_size=batch_size,
        separator=separator,
        lyrics_path=lyrics_path,
        language_override=language_override,
        whisper_model=lambda: _get_whisper(model_name, actual_device, c_type),
        pre_align_cleanup=_clear_models,
        free_gpu_fn=lambda: free_gpu(),
    )


def _send(wfile, payload):
    try:
        wfile.write(json.dumps(payload, ensure_ascii=False) + "\n")
        wfile.flush()
    except (BrokenPipeError, OSError) as e:
        print(f"[nightingale:LOG] Failed to send message: {e}", file=sys.stderr, flush=True)


def main():
    device = detect_device()
    token = secrets.token_hex(16)

    srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    srv.bind(("127.0.0.1", 0))
    srv.listen(1)
    port = srv.getsockname()[1]

    print(
        json.dumps({"event": "ready", "port": port, "token": token, "device": device}),
        flush=True,
    )

    srv.settimeout(60.0)
    try:
        conn, _addr = srv.accept()
    except socket.timeout:
        print("[nightingale:LOG] Timed out waiting for client connection", file=sys.stderr, flush=True)
        return
    finally:
        srv.close()

    conn.settimeout(None)
    rfile = conn.makefile("r", encoding="utf-8", newline="\n")
    wfile = conn.makefile("w", encoding="utf-8", newline="\n")

    hello_line = rfile.readline()
    if not hello_line:
        return
    try:
        hello = json.loads(hello_line)
    except json.JSONDecodeError:
        return
    if hello.get("type") != "hello" or hello.get("token") != token:
        print("[nightingale:LOG] Auth failed, closing connection", file=sys.stderr, flush=True)
        return
    _send(wfile, {"type": "hello_ack"})

    set_progress_sink(lambda pct, msg: _send(wfile, {"type": "progress", "pct": int(pct), "msg": str(msg)}))

    try:
        for line in rfile:
            line = line.strip()
            if not line:
                continue
            try:
                cmd = json.loads(line)
            except json.JSONDecodeError as e:
                _send(wfile, {"type": "error", "kind": "generic", "msg": f"Invalid JSON: {e}"})
                continue

            ctype = cmd.get("type")
            if ctype == "quit":
                break
            if ctype == "analyze":
                try:
                    process_song(cmd, device)
                    _send(wfile, {"type": "done", "hash": cmd.get("hash", "")})
                except Exception as e:
                    import traceback
                    traceback.print_exc(file=sys.stderr)
                    err_str = str(e)
                    if is_oom(err_str):
                        _clear_models()
                        _send(wfile, {"type": "error", "kind": "oom", "msg": err_str})
                    else:
                        _send(wfile, {"type": "error", "kind": "generic", "msg": err_str})
            else:
                _send(wfile, {"type": "error", "kind": "generic", "msg": f"Unknown command: {ctype!r}"})
    finally:
        try:
            wfile.close()
        except Exception:
            pass
        try:
            rfile.close()
        except Exception:
            pass
        try:
            conn.close()
        except Exception:
            pass


if __name__ == "__main__":
    main()
