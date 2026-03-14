# Building from Source

## Prerequisites

| Tool | Version |
|---|---|
| Rust | 1.85+ (edition 2024) |
| Linux only | `libasound2-dev`, `libudev-dev`, `libwayland-dev`, `libxkbcommon-dev` |

## Development Build

```bash
git clone <repo-url> nightingale
cd nightingale
cargo build --release
```

## Local Release

### Linux / macOS

```bash
scripts/make-release.sh
```

Builds the release binary and packages it into `nightingale-<target>.tar.gz`.

### Windows (PowerShell)

```powershell
powershell -ExecutionPolicy Bypass -File scripts/make-release.ps1
```

Builds the release binary and packages it into `nightingale-x86_64-pc-windows-msvc.zip`.

## CLI Flags

| Flag | Description |
|---|---|
| `--setup` | Force re-run of the first-launch bootstrap |
