#!/usr/bin/env bash
#
# Nightingale self-host installer for Linux.
#
# Default mode (release): downloads a prebuilt binary from a GitHub Release.
#
#   curl -fsSL https://raw.githubusercontent.com/rzru/nightingale/master/scripts/install.sh | bash
#
# Source mode (--from-source): compiles the binary from a local checkout.
# Auto-detected when run from inside a clone; pass --from-source=PATH or set
# NIGHTINGALE_SOURCE=/path to point at a different location.
#
#   bash scripts/install.sh --from-source
#
# Idempotent in either mode: re-running upgrades the binary in place and
# restarts services only when their config actually changed. No data or
# songs folders are baked in - both are picked from inside the app's setup
# wizard the first time you visit http://<host>.local.
#
# Interactive setup: the installer prompts for every configurable value up
# front - press Enter to accept the suggested default. For unattended
# installs (CI, automation, re-running with the same answers) pre-set the
# matching env var and that prompt is skipped silently:
#
#   NIGHTINGALE_REPO       "owner/repo" for the GH release source (release mode)
#   NIGHTINGALE_VERSION    git tag (e.g. v0.6.0) or "latest"      (release mode)
#   NIGHTINGALE_SOURCE     /path/to/checkout                       (source mode)
#   NIGHTINGALE_USER       system user the service runs as
#   NIGHTINGALE_HOSTNAME   mDNS name to publish on the LAN (e.g. nightingale.local)
#   NIGHTINGALE_DATA_DIR   where config.json + the published trust-root cert live
#                          (default: /var/lib/nightingale; re-runs over an
#                          existing install pick up the unit's previously
#                          chosen NIGHTINGALE_DATA_PATH instead)
#   NIGHTINGALE_REF        git ref used to fetch companion assets in curl-pipe mode
#   NIGHTINGALE_PROTECT_HOME
#                          systemd ProtectHome= for the service unit
#                          (off | read-only | tmpfs). Defaults to "off" when
#                          DATA_DIR is under /home (so uv / pip / etc. can
#                          write their per-user caches in $HOME/.cache),
#                          and "read-only" otherwise.
#   NIGHTINGALE_FORCE_CADDYFILE
#                          set to 1 to overwrite a non-managed /etc/caddy/Caddyfile
#                          (or any colliding Caddyfile.d snippet) - the operator's
#                          existing main file is still backed up to *.nightingale.bak
#   NIGHTINGALE_FORCE_AVAHI_HOSTNAME
#                          set to 1 to overwrite an existing avahi host-name= override
#
# Build prerequisites for source mode (in the *invoking* user's login shell,
# not root's): cargo, node, pnpm. rustup / fnm / mise / nvm / asdf are all
# fine - when launched through sudo, the script re-execs the build through
# `sudo -u $SUDO_USER -H -- bash -ilc` so both ~/.bash_profile (login)
# and ~/.bashrc (interactive) load,
# which is the only way to pick up tool managers gated behind the common
# `[[ $- != *i* ]] && return` guard at the top of ~/.bashrc.

set -euo pipefail

# ── Globals (filled in by configure() / parse_args()) ──────────────────

MODE=""                # "release" | "source"
REPO=""                # release mode
VERSION=""             # release mode
SOURCE_DIR=""          # source mode
SERVICE_USER=""
HOSTNAME_LABEL=""
DATA_DIR=""
PROTECT_HOME=""        # systemd ProtectHome= value, derived from DATA_DIR

readonly DEFAULT_REPO="rzru/nightingale"
readonly DEFAULT_VERSION="latest"
readonly DEFAULT_USER="nightingale"
readonly DEFAULT_DATA_DIR="/var/lib/nightingale"

readonly BIN_PATH="/usr/local/bin/nightingale"
readonly BIN_ETAG_PATH="${BIN_PATH}.etag"
readonly BIN_VERSION_LEGACY_PATH="${BIN_PATH}.version"
readonly UNIT_PATH="/etc/systemd/system/nightingale.service"
readonly CADDYFILE_PATH="/etc/caddy/Caddyfile"
readonly CADDYFILE_DROPIN_DIR="/etc/caddy/Caddyfile.d"
readonly CADDYFILE_DROPIN_PATH="${CADDYFILE_DROPIN_DIR}/nightingale.caddy"
# Trust root lives under /etc/caddy - the same directory caddy already
# reads its config from, so the `caddy` system user always has read +
# traverse access regardless of how the operator chose DATA_DIR. This
# also keeps the data dir off caddy's read path entirely, so an operator
# can put DATA_DIR under a 0700 user home without the installer having
# to mutate filesystem permissions.
readonly ROOT_CRT_PATH="/etc/caddy/nightingale-root.crt"
readonly AVAHI_SERVICE_PATH="/etc/avahi/services/nightingale.service"
readonly AVAHI_DAEMON_CONF="/etc/avahi/avahi-daemon.conf"

# Sentinel header we stamp on Caddyfile / dropin files we own. Re-runs use
# it to tell our managed file apart from one the operator has taken over.
# Kept in sync with the first line of scripts/Caddyfile.
readonly CADDY_MANAGED_HEADER="# managed-by: nightingale-installer"

# Dirty flags. Each step that mutates state flips the corresponding flag,
# and start_services() then restarts only the daemons that were actually
# touched. Re-runs over a healthy host stop bouncing caddy / avahi for fun.
CADDY_DIRTY=0
AVAHI_DIRTY=0
NIGHTINGALE_DIRTY=0
NIGHTINGALE_OWNS_HTTP_PORTS=0

# Set by render_with_overrides() / set_ini_kv() / unset_ini_kv() so callers
# can fold the change-bit into the right *_DIRTY flag.
RENDERED_CHANGED=0
INI_KV_CHANGED=0

# ── ANSI + logging helpers ─────────────────────────────────────────────

if [[ -t 1 && -z "${NO_COLOR:-}" ]]; then
  C_BOLD=$'\e[1m'; C_DIM=$'\e[2m'; C_RED=$'\e[31m'; C_GRN=$'\e[32m'
  C_YEL=$'\e[33m'; C_CYA=$'\e[36m'; C_RST=$'\e[0m'
else
  C_BOLD=""; C_DIM=""; C_RED=""; C_GRN=""; C_YEL=""; C_CYA=""; C_RST=""
fi

log()  { printf '%s==>%s %s\n' "$C_CYA" "$C_RST" "$*"; }
ok()   { printf '%s ok%s  %s\n' "$C_GRN" "$C_RST" "$*"; }
warn() { printf '%swarn%s %s\n' "$C_YEL" "$C_RST" "$*" >&2; }
die()  { printf '%serr%s  %s\n' "$C_RED" "$C_RST" "$*" >&2; exit 1; }

have_cmd() { command -v "$1" >/dev/null 2>&1; }

ROOT_CONFIRMED=0

quote_cmd() {
  local arg out=""
  for arg in "$@"; do
    printf -v arg '%q' "$arg"
    out+=" ${arg}"
  done
  printf '%s' "${out# }"
}

confirm_root_command() {
  local rendered="$1" ans=""
  [[ $EUID -eq 0 || $ROOT_CONFIRMED -eq 1 ]] && return
  if [[ -r /dev/tty ]]; then
    cat >/dev/tty <<EOF

${C_BOLD}Admin access needed.${C_RST}
This installer only uses sudo for system changes: package install, /usr/local/bin,
/etc config, /var/lib data dir, and systemd service commands.

First sudo command:
  sudo ${rendered}

EOF
    read -r -p "Continue? [Y/n] " ans </dev/tty || true
    [[ "$ans" =~ ^[Nn]$ ]] && die "aborted before sudo"
  else
    warn "admin access needed; no tty available for confirmation before sudo"
  fi
  ROOT_CONFIRMED=1
}

as_root() {
  local rendered
  rendered="$(quote_cmd "$@")"
  if [[ $EUID -eq 0 ]]; then
    printf '%s+%s %s\n' "$C_DIM" "$C_RST" "$rendered" >&2
    "$@"
  else
    confirm_root_command "$rendered"
    printf '%s+%s sudo %s\n' "$C_DIM" "$C_RST" "$rendered" >&2
    sudo "$@"
  fi
}

capture_as_root() {
  local rendered
  rendered="$(quote_cmd "$@")"
  if [[ $EUID -eq 0 ]]; then
    printf '%s+%s %s\n' "$C_DIM" "$C_RST" "$rendered" >&2
    "$@"
  else
    confirm_root_command "$rendered"
    printf '%s+%s sudo %s\n' "$C_DIM" "$C_RST" "$rendered" >&2
    sudo "$@"
  fi
}

require_privilege_tool() {
  if [[ $EUID -ne 0 ]] && ! have_cmd sudo; then
    die "sudo not found; install sudo or run this installer as root"
  fi
}

# ── Arg parsing ────────────────────────────────────────────────────────

usage() {
  cat <<EOF
Nightingale self-hosted web installer.

Usage:
  bash $(basename "${BASH_SOURCE[0]:-$0}") [--from-source[=PATH]] [--release] [-h]

Default: download a prebuilt binary from a GitHub Release.
With --from-source (or when run from inside a Nightingale clone): compile locally.

Options:
  --from-source[=PATH]   Build from a local checkout. PATH defaults to the
                         directory containing this script or \$NIGHTINGALE_SOURCE.
  --release              Force release-download mode (overrides auto-detection).
  -h, --help             This help.

Every prompt accepts an env override (NIGHTINGALE_REPO, NIGHTINGALE_VERSION,
NIGHTINGALE_USER, NIGHTINGALE_HOSTNAME, NIGHTINGALE_DATA_DIR, NIGHTINGALE_SOURCE)
to skip the prompt and run unattended.
EOF
}

parse_args() {
  local arg
  while (( $# > 0 )); do
    arg="$1"
    case "$arg" in
      --from-source)        MODE=source; shift ;;
      --from-source=*)      MODE=source; SOURCE_DIR="${arg#--from-source=}"; shift ;;
      --release)            MODE=release; shift ;;
      -h|--help)            usage; exit 0 ;;
      *)                    usage >&2; die "unknown arg: $arg" ;;
    esac
  done
}

# ── Step framework ─────────────────────────────────────────────────────
#
# build_steps() fills STEPS *after* configure() so the plan we print to the
# operator references the actually chosen hostname, data dir, etc. (not
# the prompts' placeholder defaults). step() is then called once per phase
# inside main(), in declaration order - it prints a `[N/M] <title>` header
# and asserts we never walk off the end of the array.

STEPS=()
STEP_INDEX=0

# Render a value for either the pre-prompt plan or the during-execution
# step headers:
#
#   mode == "preview"  -> show ENV_NAME's value verbatim if the operator
#                         pre-pinned it (no prompt will fire for that var,
#                         so what they see is what they'll get); otherwise
#                         show PLACEHOLDER so they know the value will be
#                         chosen at an upcoming prompt.
#
#   mode == "resolved" -> always show ACTUAL.
plan_value() {
  local mode="$1" env_name="$2" actual="$3" placeholder="$4"
  if [[ "$mode" == "resolved" || -n "${!env_name:-}" ]]; then
    printf '%s' "$actual"
  else
    printf '%s' "$placeholder"
  fi
}

step() {
  STEP_INDEX=$((STEP_INDEX + 1))
  local total="${#STEPS[@]}"
  if (( STEP_INDEX > total )); then
    die "internal: step() called ${STEP_INDEX} times but STEPS has only ${total} entries"
  fi
  local title="${STEPS[STEP_INDEX-1]}"
  printf '\n%s[%d/%d] %s%s\n' \
    "$C_BOLD" "$STEP_INDEX" "$total" "$title" "$C_RST"
}

print_banner() {
  if [[ "$MODE" == "source" ]]; then
    printf '\n%sNightingale self-hosted web installer%s %s(build from source)%s\n\n' \
      "$C_BOLD" "$C_RST" "$C_DIM" "$C_RST"
    printf '%sBuilds nightingale from a local clone and installs it as a systemd%s\n' "$C_DIM" "$C_RST"
    printf '%sservice on this Linux host, fronted by Caddy and advertised as%s\n' "$C_DIM" "$C_RST"
  else
    printf '\n%sNightingale self-hosted web installer%s\n\n' "$C_BOLD" "$C_RST"
    printf '%sInstalls nightingale.service on this Linux host, fronted by Caddy and%s\n' "$C_DIM" "$C_RST"
    printf '%sadvertised as <host>.local over mDNS. Any device on your LAN then%s\n' "$C_DIM" "$C_RST"
  fi
  printf '%sopens http://<host>.local. Browsers will tag the site "Not Secure" -%s\n' "$C_DIM" "$C_RST"
  printf '%sthat is OK for everything except secure-context-only browser APIs (mic%s\n' "$C_DIM" "$C_RST"
  printf '%scapture, clipboard, fullscreen). Install the LAN root cert once per%s\n' "$C_DIM" "$C_RST"
  printf '%sdevice (instructions print at the end) to flip those on via HTTPS.%s\n\n' "$C_DIM" "$C_RST"
  printf '%sDocs: https://nightingale.cafe/docs/self-hosted.html%s\n' "$C_DIM" "$C_RST"
}

print_plan() {
  printf '\n%sPlan%s %s(%d steps - re-runs are idempotent)%s\n\n' \
    "$C_BOLD" "$C_RST" "$C_DIM" "${#STEPS[@]}" "$C_RST"
  local i=0 title
  for title in "${STEPS[@]}"; do
    i=$((i + 1))
    printf '  %s%2d.%s %s\n' "$C_DIM" "$i" "$C_RST" "$title"
  done
  printf '\n%sValues in <angle brackets> will come from the prompts that follow;%s\n' "$C_DIM" "$C_RST"
  printf '%severything else is fixed (env override or constant).%s %sPress Ctrl+C now%s\n' "$C_DIM" "$C_RST" "$C_DIM" "$C_RST"
  printf '%s(or at any prompt) to abort - nothing on the host has changed yet.%s\n' "$C_DIM" "$C_RST"
}

# ── Interactive prompts ────────────────────────────────────────────────
#
# Usage: prompt OUT_VAR ENV_OVERRIDE_NAME "Label" "default"
#
# When ENV_OVERRIDE_NAME is already set we use that value verbatim and skip
# the prompt. Otherwise we read from /dev/tty (not stdin, because under
# `curl | bash` stdin is the curl pipe and would silently EOF). If
# /dev/tty isn't readable either (truly headless), we fall back to the
# default and announce it so the run log shows what was used.
prompt() {
  local out_var="$1" env_name="$2" label="$3" default="$4"
  local override="${!env_name:-}" ans=""
  if [[ -n "$override" ]]; then
    printf '  %s%s%s = %s %s($%s)%s\n' \
      "$C_BOLD" "$label" "$C_RST" "$override" "$C_DIM" "$env_name" "$C_RST"
    printf -v "$out_var" '%s' "$override"
    return
  fi
  if [[ -r /dev/tty ]]; then
    read -r -p "$(printf '  %s%s%s [%s]: ' "$C_BOLD" "$label" "$C_RST" "$default")" ans </dev/tty || true
    printf -v "$out_var" '%s' "${ans:-$default}"
  else
    printf '  %s%s%s = %s %s(non-interactive default)%s\n' \
      "$C_BOLD" "$label" "$C_RST" "$default" "$C_DIM" "$C_RST"
    printf -v "$out_var" '%s' "$default"
  fi
}

# Read NIGHTINGALE_DATA_PATH from a previously-installed nightingale.service
# unit so a re-run keeps the operator's existing data dir even after we
# moved the default (legacy installs lived at /home/<user>/.nightingale).
read_existing_data_dir() {
  [[ -f "$UNIT_PATH" ]] || return 1
  local path
  path="$(sed -n 's|^[[:space:]]*Environment=NIGHTINGALE_DATA_PATH=\(.*\)|\1|p' "$UNIT_PATH" | head -n1)"
  [[ -n "$path" ]] || return 1
  printf '%s' "$path"
}

# Probe for a Nightingale checkout next to (or one level above) this
# script. Empty output means no candidate was found - configure()'s prompt
# then asks the operator for one.
detect_source_dir() {
  local self
  self="${BASH_SOURCE[0]:-$0}"
  if [[ -z "$self" || ! -f "$self" ]]; then return; fi
  local script_dir
  script_dir="$(cd "$(dirname "$self")" && pwd)"
  local candidate
  for candidate in "$script_dir" "$(dirname "$script_dir")"; do
    if [[ -f "$candidate/Cargo.toml" && -d "$candidate/client/src-server" ]]; then
      echo "$candidate"
      return
    fi
  done
}

resolve_defaults() {
  # Mode auto-detection: explicit --release/--from-source wins; then env;
  # then "are we standing inside a clone?".
  if [[ -z "$MODE" ]]; then
    if [[ -n "${NIGHTINGALE_SOURCE:-}" ]]; then
      MODE=source
    else
      local candidate
      candidate="$(detect_source_dir)"
      if [[ -n "$candidate" ]]; then
        MODE=source
        SOURCE_DIR="$candidate"
      else
        MODE=release
      fi
    fi
  fi
  if [[ "$MODE" == "source" && -z "$SOURCE_DIR" ]]; then
    SOURCE_DIR="${NIGHTINGALE_SOURCE:-$(detect_source_dir || true)}"
  fi

  REPO="${NIGHTINGALE_REPO:-$DEFAULT_REPO}"
  VERSION="${NIGHTINGALE_VERSION:-$DEFAULT_VERSION}"
  SERVICE_USER="${NIGHTINGALE_USER:-$DEFAULT_USER}"
  HOSTNAME_LABEL="${NIGHTINGALE_HOSTNAME:-$(hostname -s).local}"

  # DATA_DIR precedence: explicit env override > existing unit's
  # NIGHTINGALE_DATA_PATH (so legacy installs keep working on re-run) >
  # new default.
  if [[ -n "${NIGHTINGALE_DATA_DIR:-}" ]]; then
    DATA_DIR="$NIGHTINGALE_DATA_DIR"
  else
    local existing
    existing="$(read_existing_data_dir 2>/dev/null || true)"
    if [[ -n "$existing" ]]; then
      DATA_DIR="$existing"
    else
      DATA_DIR="$DEFAULT_DATA_DIR"
    fi
  fi

  # Pick a systemd ProtectHome= value that matches where DATA_DIR lives.
  # When the operator parks data under /home/* (real-user install) the
  # service user is almost always that human's account, and any tool we
  # shell out to (uv, ffmpeg, pip) defaults its caches to $HOME/.cache.
  # ProtectHome=read-only would make those write attempts hit EROFS and
  # abort the in-app vendor setup. For the standard /var/lib/nightingale
  # case we keep the hardened default. Operators can pin it explicitly
  # with NIGHTINGALE_PROTECT_HOME=off|read-only|tmpfs.
  if [[ -n "${NIGHTINGALE_PROTECT_HOME:-}" ]]; then
    PROTECT_HOME="$NIGHTINGALE_PROTECT_HOME"
  elif [[ "$DATA_DIR" == /home/* ]]; then
    PROTECT_HOME="off"
  else
    PROTECT_HOME="read-only"
  fi
}

configure() {
  printf '\n%sConfigure install%s %s(press Enter to accept the default, Ctrl+C to abort)%s\n\n' \
    "$C_BOLD" "$C_RST" "$C_DIM" "$C_RST"

  if [[ "$MODE" == "source" ]]; then
    prompt SOURCE_DIR     NIGHTINGALE_SOURCE   "Local Nightingale repo to build from" "$SOURCE_DIR"
  else
    prompt REPO           NIGHTINGALE_REPO     "GitHub repo (owner/name)"           "$REPO"
    prompt VERSION        NIGHTINGALE_VERSION  "Release version (or 'latest')"      "$VERSION"
  fi

  prompt SERVICE_USER     NIGHTINGALE_USER     "System user for the service"        "$SERVICE_USER"
  prompt HOSTNAME_LABEL   NIGHTINGALE_HOSTNAME "mDNS name (.local)"                  "$HOSTNAME_LABEL"
  prompt DATA_DIR         NIGHTINGALE_DATA_DIR "Data dir (app config + cache)"        "$DATA_DIR"
  printf '\n'

  if [[ "$MODE" == "source" ]]; then
    if [[ -z "$SOURCE_DIR" ]]; then
      die "no Nightingale checkout supplied - re-run from inside a clone, set NIGHTINGALE_SOURCE=/path/to/repo, or pass --from-source=/path"
    fi
    if [[ ! -f "$SOURCE_DIR/Cargo.toml" || ! -d "$SOURCE_DIR/client/src-server" ]]; then
      die "'$SOURCE_DIR' does not look like a Nightingale checkout (missing Cargo.toml or client/src-server)"
    fi
  fi
}

# ── Environment checks ─────────────────────────────────────────────────

require_systemd() {
  if ! command -v systemctl >/dev/null 2>&1; then
    die "systemctl not found; this installer needs a systemd-based distro"
  fi
  if [[ ! -d /run/systemd/system ]]; then
    die "systemd is not the active init; cannot enable services"
  fi
}

detect_arch() {
  case "$(uname -m)" in
    x86_64|amd64)        echo "x86_64-unknown-linux-gnu" ;;
    aarch64|arm64)       echo "aarch64-unknown-linux-gnu" ;;
    *) die "unsupported architecture: $(uname -m)" ;;
  esac
}

# ── Package install ────────────────────────────────────────────────────
#
# Distro-agnostic: instead of allow-listing distros by `/etc/os-release ID`
# we probe for a package manager command we know how to drive. Pre-installed
# caddy + avahi-daemon (NixOS, custom builds, distros we haven't enumerated)
# are detected up-front and the install step is skipped entirely.

# True iff caddy is on PATH AND avahi-daemon is either on PATH or installed
# as a systemd unit (the binary lives in /usr/lib/avahi on some distros and
# isn't always exported to PATH, so the unit file is the more reliable
# signal that it's installed and runnable).
#
# We grep the output of `list-unit-files` rather than relying on its exit
# code: older systemd (<v245) returns 0 with empty output when no unit
# matches, which would have given us a false positive.
have_required_runtime() {
  have_cmd caddy || return 1
  have_cmd avahi-daemon && return 0
  systemctl list-unit-files avahi-daemon.service 2>/dev/null \
    | grep -qE '^avahi-daemon\.service[[:space:]]'
}

apt_add_caddy_repo() {
  # Caddy isn't in Debian stable, and the Ubuntu-shipped version is often
  # too old. The Cloudsmith repo is the upstream-blessed path documented
  # at https://caddyserver.com/docs/install#debian-ubuntu-raspbian.
  if [[ -f /usr/share/keyrings/caddy-stable-archive-keyring.gpg ]]; then
    return
  fi
  log "adding Caddy APT repo (cloudsmith)"
  as_root apt-get update -y >/dev/null
  as_root apt-get install -y debian-keyring debian-archive-keyring apt-transport-https curl gnupg >/dev/null
  local tmp
  tmp="$(mktemp -d)"
  curl -fsSL 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' -o "$tmp/caddy.gpg.key"
  as_root gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg "$tmp/caddy.gpg.key"
  curl -fsSL 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' -o "$tmp/caddy-stable.list"
  as_root install -m 0644 -o root -g root "$tmp/caddy-stable.list" /etc/apt/sources.list.d/caddy-stable.list
  rm -rf "$tmp"
  as_root apt-get update -y >/dev/null
}

install_packages_apt() {
  export DEBIAN_FRONTEND=noninteractive
  as_root apt-get update -y >/dev/null
  as_root apt-get install -y --no-install-recommends ca-certificates curl tar avahi-daemon >/dev/null
  if have_cmd caddy; then return; fi
  # Prefer the distro-shipped caddy when it's actually packaged. Adding
  # the Cloudsmith apt source is a one-way change to the operator's apt
  # state, so we only do it when the distro genuinely doesn't ship caddy.
  if apt-cache show caddy 2>/dev/null | grep -q '^Package: caddy$'; then
    as_root apt-get install -y caddy >/dev/null
    return
  fi
  apt_add_caddy_repo
  as_root apt-get install -y caddy >/dev/null
}

install_packages_dnf() {
  as_root dnf install -y --setopt=install_weak_deps=False ca-certificates curl tar caddy avahi >/dev/null
}

install_packages_pacman() {
  as_root pacman -Sy --needed --noconfirm ca-certificates curl tar caddy avahi >/dev/null
}

install_packages_zypper() {
  as_root zypper --non-interactive install --no-recommends ca-certificates curl tar caddy avahi >/dev/null
}

install_packages_apk() {
  # `shadow` ships useradd/groupadd; stock Alpine only has BusyBox's
  # adduser, which we don't drive. Cheaper to pull shadow on the apk
  # path than to grow a second user-creation code path.
  as_root apk add --no-cache ca-certificates curl tar shadow caddy avahi >/dev/null
}

print_manual_install_help() {
  cat >&2 <<EOF

${C_BOLD}Install caddy and avahi-daemon manually, then re-run this script.${C_RST}

  ${C_DIM}Caddy${C_RST}            https://caddyserver.com/docs/install
  ${C_DIM}avahi-daemon${C_RST}     usually 'avahi' or 'avahi-daemon' in your distro's repos

Once both binaries are on PATH (or installed as systemd units), this
installer will skip the package step entirely on the next run.

EOF
}

install_packages() {
  if have_required_runtime; then
    ok "caddy + avahi-daemon already installed; skipping package step"
    return
  fi
  if   have_cmd apt-get; then install_packages_apt
  elif have_cmd dnf;     then install_packages_dnf
  elif have_cmd pacman;  then install_packages_pacman
  elif have_cmd zypper;  then install_packages_zypper
  elif have_cmd apk;     then install_packages_apk
  else
    print_manual_install_help
    die "no supported package manager found (looked for apt-get, dnf, pacman, zypper, apk)"
  fi
}

# ── Source-mode build ──────────────────────────────────────────────────

# Run a command from `cwd` as the user that invoked sudo, falling back to
# the current (root) context when the script wasn't called through sudo.
#
# Two reasons we don't use the obvious `sudo -iu USER -- bash -lc ...`
# here:
#
#   1. `sudo -i` joins every trailing arg into a single shell -c string
#      and re-escapes the result. That mangles the nested quoting in our
#      command snippet (`'command -v "$1"'` ends up with the inner quotes
#      stripped, so the shell tries to run `command -v $1 ...` with $1
#      empty). `sudo -u USER -H -- ...` instead exec()s bash with our
#      argv intact - each positional parameter survives.
#
#   2. `bash -lc` (login, non-interactive) only sources ~/.bash_profile.
#      Tool managers like mise / asdf / nvm are usually activated from
#      ~/.bashrc, which on most distros (including Omarchy / many Arch
#      defaults) opens with `[[ $- != *i* ]] && return` - the early-out
#      that skips it for non-interactive shells. `bash -ilc` (login +
#      interactive) sources ~/.bashrc too, satisfying that gate so the
#      tool managers actually load and put their shims on PATH.
#
# Cosmetic side-effect: bash interactive mode without a controlling tty
# prints "bash: cannot set terminal process group" / "no job control in
# this shell" to stderr at startup. They're harmless and tolerated. The
# probe path silences them; the build path passes stderr through so real
# compile errors are still visible.
run_as_invoker() {
  local cwd="$1"; shift
  if [[ -n "${SUDO_USER:-}" && "${SUDO_USER}" != "root" ]]; then
    sudo -u "$SUDO_USER" -H -- bash -ilc 'cd "$1" && shift && exec "$@"' \
      bash "$cwd" "$@" </dev/null
  else
    (cd "$cwd" && "$@")
  fi
}

# Probe `cmd` in the invoker's full login + interactive shell context
# so PATH includes rustup / fnm / mise / asdf / etc. Stderr from shell
# init is silenced - we only care about the exit code.
invoker_has_cmd() {
  if [[ -n "${SUDO_USER:-}" && "${SUDO_USER}" != "root" ]]; then
    sudo -u "$SUDO_USER" -H -- bash -ilc 'command -v "$1" >/dev/null 2>&1' \
      bash "$1" </dev/null 2>/dev/null
  else
    command -v "$1" >/dev/null 2>&1
  fi
}

require_build_toolchain() {
  local missing=() cmd
  for cmd in cargo node pnpm; do
    if ! invoker_has_cmd "$cmd"; then
      missing+=("$cmd")
    fi
  done
  if (( ${#missing[@]} > 0 )); then
    local who
    if [[ -n "${SUDO_USER:-}" ]]; then who="${SUDO_USER}'s"; else who="root's"; fi
    die "missing build tools in ${who} login shell: ${missing[*]} - install them (rustup, node, pnpm) or omit --from-source to download a release"
  fi
}

build_from_source() {
  local src="$1"
  require_build_toolchain

  log "building from source: $src"

  # rust-embed slurps client/dist/ at compile time and only reruns its
  # build script when files inside dist/ change, not when the TypeScript
  # sources change. Always rebuild the frontend so re-runs reliably pick
  # up source edits.
  log "building frontend bundle (pnpm install + build)"
  run_as_invoker "$src/client" pnpm install --frozen-lockfile
  run_as_invoker "$src/client" pnpm build

  log "compiling server binary (cargo build --release -p server)"
  run_as_invoker "$src" cargo build --release --locked -p server

  local built="$src/target/release/server"
  if [[ ! -x "$built" ]]; then
    die "cargo finished but $built is missing; check the build log above"
  fi

  as_root install -m 0755 -o root -g root "$built" "$BIN_PATH"
  # Source builds embed timestamps, so the on-disk binary will differ from
  # a previous run even when no source changed. Treat every successful
  # build as "binary changed" rather than running cmp against a multi-MB
  # file; the operator opted into source mode for a reason.
  NIGHTINGALE_DIRTY=1
  # Wipe any stale ETag from a prior release-mode install so the next
  # release-mode rerun doesn't think this source build is the cached
  # version of some github asset.
  as_root rm -f "$BIN_ETAG_PATH" "$BIN_VERSION_LEGACY_PATH"
  ok "installed $BIN_PATH (from source $src)"
}

# ── Release-mode download ──────────────────────────────────────────────

resolve_download_url() {
  local target="$1" version="$2"
  if [[ "$version" == "latest" ]]; then
    echo "https://github.com/${REPO}/releases/latest/download/nightingale-server-${target}.tar.gz"
  else
    echo "https://github.com/${REPO}/releases/download/${version}/nightingale-server-${target}.tar.gz"
  fi
}

# Read the last `etag:` header value from a curl-saved header dump. The
# final (post-redirect) ETag is what S3 puts on the actual asset, so this
# is the value worth comparing across runs.
extract_last_etag() {
  awk 'BEGIN{IGNORECASE=1} /^etag:/ {
         val=$0; sub(/^[Ee][Tt][Aa][Gg]:[[:space:]]*/, "", val); gsub(/\r/, "", val)
         etag=val
       }
       END { print etag }' "$1"
}

download_binary() {
  local target url tmp tarball sha_url sha_file
  target="$(detect_arch)"
  url="$(resolve_download_url "$target" "$VERSION")"
  tmp="$(mktemp -d)"
  tarball="$tmp/nightingale-server.tar.gz"

  # Cheap conditional GET via ETag: HEAD the URL, compare to the ETag we
  # recorded last time we installed. On match (and the binary is still on
  # disk) skip the download + reinstall entirely. Works for "latest" too,
  # because GitHub's S3 backend returns an ETag tied to the actual asset
  # contents - so when "latest" hasn't moved, ETags match.
  if [[ -x "$BIN_PATH" && -f "$BIN_ETAG_PATH" ]]; then
    local remote_etag local_etag
    curl -fsSIL --max-time 10 "$url" -o "$tmp/head" 2>/dev/null || true
    remote_etag=""
    if [[ -s "$tmp/head" ]]; then
      remote_etag="$(extract_last_etag "$tmp/head")"
    fi
    local_etag="$(<"$BIN_ETAG_PATH")"
    if [[ -n "$remote_etag" && "$remote_etag" == "$local_etag" ]]; then
      rm -rf "$tmp"
      ok "binary unchanged on remote (etag match); skipping download"
      return
    fi
  fi

  log "downloading $url"
  if ! curl -fSL --retry 3 --retry-delay 2 -D "$tmp/headers" -o "$tarball" "$url"; then
    rm -rf "$tmp"
    die "failed to download release artifact - check NIGHTINGALE_VERSION (got '$VERSION') and your network, or pass --from-source from a clone"
  fi

  sha_url="${url}.sha256"
  sha_file="$tmp/nightingale-server.tar.gz.sha256"
  if curl -fsSL -o "$sha_file" "$sha_url" 2>/dev/null; then
    # GitHub's sidecar is generated next to the release asset, so its
    # filename is `nightingale-server-${target}.tar.gz`. We download to a
    # stable temp filename; verify the digest while substituting that local
    # basename so `sha256sum -c` does not try to open the release filename.
    (cd "$tmp" && awk -v name="$(basename "$tarball")" 'NF { print $1 "  " name; exit }' "$(basename "$sha_file")" | sha256sum -c --strict --status -) \
      || die "sha256 mismatch on downloaded tarball; refusing to install"
    ok "sha256 verified"
  else
    warn "no .sha256 sidecar published; skipping checksum"
  fi

  tar -xzf "$tarball" -C "$tmp"
  if [[ ! -x "$tmp/nightingale" ]]; then
    die "tarball did not contain a 'nightingale' executable"
  fi

  as_root install -m 0755 -o root -g root "$tmp/nightingale" "$BIN_PATH"
  NIGHTINGALE_DIRTY=1
  # Record the ETag of the artifact we just installed (atomic write via a
  # tempfile + install). No ETag on the response (rare) -> wipe any stale
  # recorded ETag so the next run doesn't shortcut against a value that
  # corresponds to an earlier asset.
  local new_etag etag_tmp
  new_etag="$(extract_last_etag "$tmp/headers")"
  if [[ -n "$new_etag" ]]; then
    etag_tmp="$(mktemp)"
    printf '%s\n' "$new_etag" > "$etag_tmp"
    as_root install -m 0644 -o root -g root "$etag_tmp" "$BIN_ETAG_PATH"
    rm -f "$etag_tmp"
  else
    as_root rm -f "$BIN_ETAG_PATH"
  fi
  # Drop the legacy `.version` sentinel from older installs so re-runs
  # leave a clean state.
  as_root rm -f "$BIN_VERSION_LEGACY_PATH"
  rm -rf "$tmp"
  ok "installed $BIN_PATH (from release ${VERSION})"
}

acquire_binary() {
  if [[ "$MODE" == "source" ]]; then
    build_from_source "$SOURCE_DIR"
  else
    download_binary
  fi
}

# ── Filesystem layout ──────────────────────────────────────────────────

ensure_user() {
  if id -u "$SERVICE_USER" >/dev/null 2>&1; then
    # Empty / nologin / false shells are service identities - attach
    # silently. A real login shell means we might be about to bind the
    # service to a human's account by accident.
    local shell
    shell="$(getent passwd "$SERVICE_USER" 2>/dev/null | cut -d: -f7)"
    case "$shell" in
      ""|*/nologin|*/false) ;;
      *)
        # The only case worth blocking is "operator accepted the default
        # `nightingale` without an explicit confirmation, AND a real
        # login user happens to live at that name" (e.g. piped install
        # on a box where someone manually created a `nightingale`
        # human account ages ago). Anything else - a different name
        # typed at the prompt, or NIGHTINGALE_USER set explicitly - is
        # a deliberate choice; let it through with a warning.
        if [[ "$SERVICE_USER" == "$DEFAULT_USER" && -z "${NIGHTINGALE_USER:-}" ]]; then
          die "default service user '$SERVICE_USER' already exists with login shell '$shell' (looks like a real account, not a service user). Refusing to attach silently. Re-run with NIGHTINGALE_USER=$SERVICE_USER to confirm, or pick a different name at the prompt."
        fi
        warn "attaching nightingale.service to existing user '$SERVICE_USER' with login shell '$shell'"
        ;;
    esac
    return
  fi
  log "creating system user $SERVICE_USER"
  # /usr/sbin/nologin (usrmerge: Debian/Ubuntu/Fedora/Arch/openSUSE/...)
  # /sbin/nologin     (Alpine and other non-usrmerged hosts)
  # /bin/false        (last-resort fallback if nologin isn't installed)
  local nologin
  for nologin in /usr/sbin/nologin /sbin/nologin /bin/false; do
    [[ -x "$nologin" ]] && break
  done
  as_root useradd --system --home-dir "$DATA_DIR" --no-create-home \
          --shell "$nologin" --comment "Nightingale karaoke server" \
          "$SERVICE_USER"
  NIGHTINGALE_DIRTY=1
}

ensure_data_dir() {
  if [[ ! -d "$DATA_DIR" ]]; then
    as_root install -d -m 0755 -o "$SERVICE_USER" -g "$SERVICE_USER" "$DATA_DIR"
    NIGHTINGALE_DIRTY=1
  else
    # Fix ownership in case the operator changed NIGHTINGALE_USER between
    # runs. Files inside are left alone - a -R chown would surprise.
    as_root chown "$SERVICE_USER":"$SERVICE_USER" "$DATA_DIR" || true
  fi
  # No caddy-traversability check here anymore: the trust root lives
  # under /etc/caddy/ (which caddy reads by definition), so DATA_DIR
  # can have whatever permissions the operator wants.
}

# ── Asset resolution ───────────────────────────────────────────────────

# Resolve where the companion files (Caddyfile, units/*) live:
#   - next to this script (cloned repo path)
#   - or under $SOURCE_DIR/scripts/ (source mode with NIGHTINGALE_SOURCE)
#   - or fetched from GitHub at NIGHTINGALE_REF (curl-pipe install).
resolve_assets_dir() {
  local self assets
  self="${BASH_SOURCE[0]:-$0}"
  if [[ -n "$self" && -f "$self" ]]; then
    assets="$(cd "$(dirname "$self")" && pwd)"
    if [[ -f "$assets/Caddyfile" && -f "$assets/units/nightingale.service" ]]; then
      echo "$assets"
      return
    fi
  fi
  if [[ "$MODE" == "source" ]]; then
    if [[ -n "$SOURCE_DIR" \
          && -f "$SOURCE_DIR/scripts/Caddyfile" \
          && -f "$SOURCE_DIR/scripts/units/nightingale.service" ]]; then
      echo "$SOURCE_DIR/scripts"
      return
    fi
    die "cannot locate companion assets (Caddyfile, units/) - expected next to this script or under \$NIGHTINGALE_SOURCE/scripts"
  fi
  # Release mode + piped install: fetch from GitHub at the same ref as the
  # script (defaults to master; override with NIGHTINGALE_REF=<tag>).
  local ref="${NIGHTINGALE_REF:-master}" tmp path
  tmp="$(mktemp -d)"
  log "fetching companion assets at ref $ref" >&2
  mkdir -p "$tmp/units"
  for path in \
      "scripts/Caddyfile" \
      "scripts/units/nightingale.service" \
      "scripts/units/avahi-nightingale.service"
  do
    curl -fsSL "https://raw.githubusercontent.com/${REPO}/${ref}/${path}" \
      -o "$tmp/${path#scripts/}" \
      || die "failed to fetch ${path} from ${REPO}@${ref}"
  done
  echo "$tmp"
}

# Backup an existing config the first time we touch it - never overwrite a
# user's hand-edited file silently.
backup_once() {
  local path="$1"
  if [[ -f "$path" && ! -f "${path}.nightingale.bak" ]]; then
    as_root cp -a "$path" "${path}.nightingale.bak"
    warn "backed up existing ${path} to ${path}.nightingale.bak"
  fi
}

# Render `src` into `dst` with @TOKEN@ substitutions, but only if the
# result differs from what's already on disk. Sets RENDERED_CHANGED=1 iff
# `dst` was actually rewritten - so callers can roll the change-bit into
# the relevant *_DIRTY flag and skip unnecessary daemon restarts.
render_with_overrides() {
  local src="$1" dst="$2" mode="$3"
  RENDERED_CHANGED=0
  local tmp
  tmp="$(mktemp)"
  sed \
    -e "s|@DATA_DIR@|${DATA_DIR}|g" \
    -e "s|@SERVICE_USER@|${SERVICE_USER}|g" \
    -e "s|@PROTECT_HOME@|${PROTECT_HOME}|g" \
    "$src" > "$tmp"
  if [[ -f "$dst" ]] && cmp -s "$tmp" "$dst"; then
    rm -f "$tmp"
    return
  fi
  as_root install -m "$mode" -o root -g root "$tmp" "$dst"
  rm -f "$tmp"
  RENDERED_CHANGED=1
}

drop_systemd_unit() {
  local assets="$1"
  render_with_overrides "$assets/units/nightingale.service" "$UNIT_PATH" 0644
  if (( RENDERED_CHANGED )); then
    NIGHTINGALE_DIRTY=1
    as_root systemctl daemon-reload
  fi
}

# ── Caddy config drops ─────────────────────────────────────────────────

# True iff `path`'s first line carries our sentinel - i.e. we wrote it on
# a previous run and the operator hasn't taken it over by hand.
caddy_file_is_managed() {
  local path="$1"
  [[ -f "$path" ]] || return 1
  head -n1 "$path" 2>/dev/null | grep -qF "$CADDY_MANAGED_HEADER"
}

# True iff `path` is owned by `package` AND has not been modified since
# the package shipped it. False on any uncertainty - the caller treats
# that as "operator-owned" and uses the additive Caddyfile.d path.
package_file_unmodified() {
  local path="$1" package="$2"

  if have_cmd dpkg-query; then
    if dpkg-query -S "$path" 2>/dev/null | grep -qE "^${package}:"; then
      # dpkg -V emits one line per modified/missing file. No line for
      # `path` -> unmodified.
      ! dpkg -V "$package" 2>/dev/null | awk '{print $NF}' | grep -qxF "$path"
      return
    fi
  fi

  if have_cmd pacman; then
    local owner
    owner="$(pacman -Qo "$path" 2>/dev/null | sed -n 's|.*is owned by \([^ ]*\) .*|\1|p')"
    if [[ "$owner" == "$package" ]]; then
      ! { pacman -Qkk "$package" 2>&1 | grep -F "$path" | grep -qi 'mismatch'; }
      return
    fi
  fi

  if have_cmd rpm; then
    local owner
    owner="$(rpm -qf --qf '%{NAME}\n' "$path" 2>/dev/null | head -n1)"
    if [[ "$owner" == "$package" ]]; then
      ! rpm -V "$package" 2>/dev/null | awk '{print $NF}' | grep -qxF "$path"
      return
    fi
  fi

  return 1
}

caddyfile_is_distro_template() {
  [[ -f "$CADDYFILE_PATH" ]] || return 1
  package_file_unmodified "$CADDYFILE_PATH" caddy
}

# True iff /etc/caddy/Caddyfile carries non-comment, non-blank content
# that we did NOT write AND that isn't an unmodified distro-shipped
# template. That's the signal to switch to additive (Caddyfile.d) mode
# instead of overwriting the operator's site config.
caddyfile_is_user_owned() {
  [[ -f "$CADDYFILE_PATH" ]] || return 1
  caddy_file_is_managed "$CADDYFILE_PATH" && return 1
  caddyfile_is_distro_template && return 1
  grep -qvE '^[[:space:]]*(#|$)' "$CADDYFILE_PATH"
}

# Walk the operator's caddy config (main file + every snippet under
# Caddyfile.d/), skipping anything we manage, and emit any path that
# defines an http://-or-:443 site block - those will collide with our
# snippet at adapt time. Used by drop_caddy_config to fail fast.
caddy_collision_files() {
  local f
  shopt -s nullglob
  for f in "$CADDYFILE_PATH" "${CADDYFILE_DROPIN_DIR}"/*.caddy; do
    [[ -f "$f" ]] || continue
    caddy_file_is_managed "$f" && continue
    if grep -qE '^[[:space:]]*(http://|:443)[[:space:]]*\{' "$f"; then
      printf '%s\n' "$f"
    fi
  done
  shopt -u nullglob
}

drop_caddy_config() {
  local assets="$1"
  local force_takeover="${NIGHTINGALE_FORCE_CADDYFILE:-0}"
  as_root install -d -m 0755 /etc/caddy

  if caddyfile_is_user_owned && [[ "$force_takeover" != "1" ]]; then
    log "existing $CADDYFILE_PATH is user-owned; installing nightingale snippet as an additive import"
    as_root install -d -m 0755 "$CADDYFILE_DROPIN_DIR"
    render_with_overrides "$assets/Caddyfile" "$CADDYFILE_DROPIN_PATH" 0644
    if (( RENDERED_CHANGED )); then CADDY_DIRTY=1; fi
    if ! grep -qE '^[[:space:]]*import[[:space:]]+Caddyfile\.d/' "$CADDYFILE_PATH"; then
      backup_once "$CADDYFILE_PATH"
      printf '\n# Added by nightingale-installer\nimport Caddyfile.d/*.caddy\n' | as_root tee -a "$CADDYFILE_PATH" >/dev/null
      log "appended 'import Caddyfile.d/*.caddy' to $CADDYFILE_PATH"
      CADDY_DIRTY=1
    fi
    # Hard-fail on duplicate listeners anywhere in the merged config (main
    # Caddyfile *or* any snippet under Caddyfile.d/, except our own).
    # caddy would otherwise refuse to adapt with an opaque "ambiguous site
    # definition: http://" - this catches it up front with a fix recipe.
    local collisions=()
    while IFS= read -r line; do
      [[ -n "$line" ]] && collisions+=("$line")
    done < <(caddy_collision_files)
    if (( ${#collisions[@]} > 0 )); then
      local msg f
      msg="the following file(s) define an http:// or :443 listener that will collide with the nightingale snippet:"$'\n'
      for f in "${collisions[@]}"; do
        msg+="  - ${f}"$'\n'
      done
      msg+=$'\n'"Two ways forward:"$'\n\n'
      msg+="  1. Move those listeners out of the colliding file(s) into their own"$'\n'
      msg+="     file under ${CADDYFILE_DROPIN_DIR}/ - caddy already imports"$'\n'
      msg+="     everything in that directory via the 'import Caddyfile.d/*.caddy'"$'\n'
      msg+="     line we just added (or appended to ${CADDYFILE_PATH}). Then"$'\n'
      msg+="     re-run this installer."$'\n\n'
      msg+="  2. Or let us take over the main Caddyfile entirely - we'll back"$'\n'
      msg+="     yours up to ${CADDYFILE_PATH}.nightingale.bak and replace it"$'\n'
      msg+="     with our config. Re-run with NIGHTINGALE_FORCE_CADDYFILE=1 set."
      die "$msg"
    fi
    return
  fi

  if [[ "$force_takeover" == "1" && -f "$CADDYFILE_PATH" ]] && ! caddy_file_is_managed "$CADDYFILE_PATH"; then
    log "NIGHTINGALE_FORCE_CADDYFILE=1; replacing $CADDYFILE_PATH with our config (backup at ${CADDYFILE_PATH}.nightingale.bak)"
  fi

  # No file, file we wrote previously, distro-shipped template, comments-
  # only, or operator opted into the force-takeover path. Render ours as
  # the main file. Drop a now-redundant managed snippet under Caddyfile.d/
  # if a previous re-run dropped one before we knew the main file was
  # safe to overwrite.
  if [[ -f "$CADDYFILE_DROPIN_PATH" ]] && caddy_file_is_managed "$CADDYFILE_DROPIN_PATH"; then
    as_root rm -f "$CADDYFILE_DROPIN_PATH"
    log "removed redundant managed snippet $CADDYFILE_DROPIN_PATH (main Caddyfile now holds the same config)"
    CADDY_DIRTY=1
  fi
  backup_once "$CADDYFILE_PATH"
  render_with_overrides "$assets/Caddyfile" "$CADDYFILE_PATH" 0644
  if (( RENDERED_CHANGED )); then CADDY_DIRTY=1; fi
}

# ── Avahi config drops ─────────────────────────────────────────────────

drop_avahi_service() {
  local assets="$1" src="$assets/units/avahi-nightingale.service"
  as_root install -d -m 0755 /etc/avahi/services
  if [[ -f "$AVAHI_SERVICE_PATH" ]] && cmp -s "$src" "$AVAHI_SERVICE_PATH"; then
    return
  fi
  as_root install -m 0644 -o root -g root "$src" "$AVAHI_SERVICE_PATH"
  AVAHI_DIRTY=1
}

# set_ini_kv FILE SECTION KEY VALUE - rewrite/insert KEY=VALUE under
# [SECTION]. Tries: rewrite the existing line; uncomment + set the
# template's commented placeholder; append after [SECTION]; otherwise
# append a fresh [SECTION] block. Sets INI_KV_CHANGED=1 iff anything
# actually moved on disk.
set_ini_kv() {
  local file="$1" section="$2" key="$3" value="$4"
  INI_KV_CHANGED=0
  local current
  current="$(sed -n "s|^[[:space:]]*${key}=[[:space:]]*\([^[:space:]#]*\).*|\1|p" "$file" | head -n1)"
  if [[ "$current" == "$value" ]]; then
    return
  fi
  if grep -qE "^[[:space:]]*${key}=" "$file"; then
    as_root sed -i "s|^[[:space:]]*${key}=.*|${key}=${value}|" "$file"
  elif grep -qE "^[[:space:]]*#${key}=" "$file"; then
    as_root sed -i "0,/^[[:space:]]*#${key}=.*/{s|^[[:space:]]*#${key}=.*|${key}=${value}|}" "$file"
  elif grep -q "^\[${section}\]" "$file"; then
    as_root sed -i "/^\[${section}\]/a ${key}=${value}" "$file"
  else
    printf '\n[%s]\n%s=%s\n' "$section" "$key" "$value" | as_root tee -a "$file" >/dev/null
  fi
  INI_KV_CHANGED=1
}

# unset_ini_kv FILE KEY - comment out an active KEY= line if present.
# No-op when the line is already commented out or missing.
unset_ini_kv() {
  local file="$1" key="$2"
  INI_KV_CHANGED=0
  if ! grep -qE "^[[:space:]]*${key}=" "$file"; then
    return
  fi
  as_root sed -i "s|^\([[:space:]]*\)${key}=|\1#${key}=|" "$file"
  INI_KV_CHANGED=1
}

# Make avahi actually advertise the hostname the operator typed at the
# prompt. By default avahi publishes whatever `gethostname()` returns,
# so e.g. `NIGHTINGALE_HOSTNAME=nightingale.local` on a box whose system
# hostname is `sora` would silently do nothing without this step.
configure_avahi_hostname() {
  local label="${HOSTNAME_LABEL%.local}"
  label="${label%.}"
  if [[ -z "$label" ]]; then
    return
  fi
  if [[ ! -f "$AVAHI_DAEMON_CONF" ]]; then
    warn "$AVAHI_DAEMON_CONF not found; cannot pin mDNS hostname to '${label}.local'"
    return
  fi

  local sys_host current_override
  sys_host="$(hostname -s 2>/dev/null || true)"
  current_override="$(sed -n 's/^[[:space:]]*host-name=[[:space:]]*\([^[:space:]#]*\).*/\1/p' "$AVAHI_DAEMON_CONF" | head -n1)"

  # Safety: leave a user-owned override alone unless the operator opts in.
  if [[ -n "$current_override" && "$current_override" != "$label" && "$current_override" != "$sys_host" ]]; then
    if [[ "${NIGHTINGALE_FORCE_AVAHI_HOSTNAME:-0}" != "1" ]]; then
      warn "avahi already publishes '${current_override}.local' (host-name= in $AVAHI_DAEMON_CONF)"
      warn "leaving the existing override alone; mDNS will NOT resolve '${label}.local' until you change it"
      warn "set NIGHTINGALE_FORCE_AVAHI_HOSTNAME=1 to overwrite the existing override"
      return
    fi
    warn "NIGHTINGALE_FORCE_AVAHI_HOSTNAME=1; replacing existing avahi host-name '${current_override}' with '${label}'"
  fi

  if [[ "$label" == "$sys_host" ]]; then
    if [[ -n "$current_override" ]]; then
      backup_once "$AVAHI_DAEMON_CONF"
      unset_ini_kv "$AVAHI_DAEMON_CONF" "host-name"
      if (( INI_KV_CHANGED )); then
        AVAHI_DIRTY=1
        log "cleared avahi host-name override (using system hostname '${sys_host}.local')"
      fi
    fi
    return
  fi

  if [[ "$current_override" == "$label" ]]; then
    return
  fi

  backup_once "$AVAHI_DAEMON_CONF"
  set_ini_kv "$AVAHI_DAEMON_CONF" server "host-name" "$label"
  if (( INI_KV_CHANGED )); then
    AVAHI_DIRTY=1
    log "pinned avahi mDNS hostname to '${label}.local'"
  fi
}

# Avahi defaults to publishing on every interface that's up. On a host
# running containers, that includes docker bridges, podman's cni*
# networks, libvirt's virbr0, lxc's lxcbr0. These are host-internal and
# not routable from the LAN; avahi cheerfully advertising on them anyway
# means `getent hosts <host>.local` returns dead bridge IPs alongside the
# real LAN address, and `curl <host>.local` hangs on TCP timeout.
#
# Tunnel interfaces (tailscale*, wg*, tun*, tap*) are deliberately NOT
# filtered - operators frequently want nightingale reachable over VPN.
configure_avahi_interfaces() {
  if [[ ! -f "$AVAHI_DAEMON_CONF" ]]; then
    return
  fi
  if ! command -v ip >/dev/null 2>&1; then
    warn "ip(1) not available; skipping avahi virtual-bridge filter (mDNS may publish on docker/podman bridges)"
    return
  fi

  local virtual_ifaces=() iface
  while IFS= read -r iface; do
    [[ -z "$iface" ]] && continue
    case "$iface" in
      docker*|br-*|virbr*|podman*|cni-*|cni0|cni1|lxcbr*)
        virtual_ifaces+=("$iface")
        ;;
    esac
  done < <(ip -o link show 2>/dev/null | awk -F': ' '{print $2}' | sed 's/@.*//')

  if (( ${#virtual_ifaces[@]} == 0 )); then
    return
  fi

  local our_deny merged_deny current_deny
  our_deny="$(IFS=,; echo "${virtual_ifaces[*]}")"
  current_deny="$(sed -n 's/^[[:space:]]*deny-interfaces=[[:space:]]*\(.*\)/\1/p' "$AVAHI_DAEMON_CONF" | head -n1)"

  if [[ -n "$current_deny" ]]; then
    # Union the existing list with ours, deduped, preserving any entries
    # the operator added that we don't recognise.
    merged_deny="$(printf '%s,%s\n' "$current_deny" "$our_deny" \
      | tr ',' '\n' | sed 's/^[[:space:]]*//;s/[[:space:]]*$//;/^$/d' \
      | sort -u | paste -sd, -)"
    if [[ "$merged_deny" == "$current_deny" ]]; then
      return
    fi
  else
    merged_deny="$our_deny"
  fi

  backup_once "$AVAHI_DAEMON_CONF"
  set_ini_kv "$AVAHI_DAEMON_CONF" server "deny-interfaces" "$merged_deny"
  if (( INI_KV_CHANGED )); then
    AVAHI_DIRTY=1
    log "filtered virtual bridges from avahi mDNS publishing: ${our_deny}"
  fi
}

# ── Pre-flight checks ──────────────────────────────────────────────────

# Caddy listens on :80 and :443. If something else (nginx, apache, another
# service entirely) already owns those ports, the `systemctl restart caddy`
# below will fail in a way that's noisy but unhelpful. Probe the bound
# listeners up front and abort with a single clear message. A prior
# nightingale.service binding those ports is OK: upgrades stop it before
# restarting caddy.
check_port_conflicts() {
  local probe_cmd=""
  if   command -v ss   >/dev/null 2>&1; then probe_cmd="ss"
  elif command -v lsof >/dev/null 2>&1; then probe_cmd="lsof"
  else
    warn "neither ss(1) nor lsof(1) available; skipping :80/:443 conflict pre-check (caddy restart below will surface any clash)"
    return
  fi

  local conflicts=() port lines
  for port in 80 443; do
    if [[ "$probe_cmd" == "ss" ]]; then
      lines="$(capture_as_root ss -tlnpH 2>/dev/null | awk -v p=":${port}\$" '$4 ~ p')"
    else
      lines="$(capture_as_root lsof -nP -iTCP:"${port}" -sTCP:LISTEN 2>/dev/null | tail -n +2)"
    fi
    [[ -z "$lines" ]] && continue

    # Caddy itself on either port is fine - that's our own listener from
    # a prior run (or another caddy site we'll share via Caddyfile.d).
    # Nightingale itself is also fine: older/self-host runs may be the
    # thing currently occupying the public ports, and this installer will
    # stop it before restarting caddy.
    # Match program names exactly, not as substrings of the full row, so
    # an unrelated argv/path containing "caddy" or "nightingale" doesn't
    # get a free pass.
    local listener_is_ours=0 listener_is_nightingale=0
    if [[ "$probe_cmd" == "ss" ]]; then
      grep -qE 'users:\(\("caddy",' <<<"$lines" && listener_is_ours=1
      grep -qE 'users:\(\("nightingale",' <<<"$lines" && listener_is_nightingale=1
    else
      awk '{print $1}' <<<"$lines" | grep -qx caddy && listener_is_ours=1
      awk '{print $1}' <<<"$lines" | grep -qx nightingale && listener_is_nightingale=1
    fi
    if (( listener_is_nightingale )); then
      NIGHTINGALE_OWNS_HTTP_PORTS=1
      listener_is_ours=1
    fi
    if (( listener_is_ours )); then
      continue
    fi

    warn "port ${port} is bound by a non-caddy/non-nightingale process:"
    while IFS= read -r row; do
      [[ -z "$row" ]] && continue
      warn "  $row"
    done <<<"$lines"
    conflicts+=("$port")
  done

  if (( ${#conflicts[@]} > 0 )); then
    die "port(s) ${conflicts[*]} are in use by another service - stop it (or move it off the standard HTTP/HTTPS ports) and re-run"
  fi
}

# ── Service lifecycle ──────────────────────────────────────────────────

# Validate the merged caddy config before we restart. Catches every shape
# of "the merged adapt now fails" - distro-template + force-takeover gone
# wrong, an additive snippet collision we missed, anything - so an
# operator's previously-running caddy is never killed by this installer.
caddy_validate_or_die() {
  if ! have_cmd caddy; then
    warn "caddy command not on PATH; skipping pre-restart config validation"
    return
  fi
  if caddy validate --config "$CADDYFILE_PATH" --adapter caddyfile >/dev/null 2>&1; then
    return
  fi
  warn "merged caddy config does not validate; leaving the running caddy alone"
  caddy validate --config "$CADDYFILE_PATH" --adapter caddyfile >&2 || true
  die "fix the caddy config (see error above) and re-run; nothing was restarted in this step"
}

start_services() {
  log "enabling and starting services"

  as_root systemctl enable avahi-daemon || true
  if (( AVAHI_DIRTY )) || ! systemctl is-active --quiet avahi-daemon; then
    as_root systemctl restart avahi-daemon \
      || warn "avahi-daemon failed to start (mDNS .local resolution will not work)"
  else
    ok "avahi-daemon config unchanged; not restarting"
  fi

  as_root systemctl enable caddy || true
  if (( CADDY_DIRTY )) || ! systemctl is-active --quiet caddy; then
    if (( NIGHTINGALE_OWNS_HTTP_PORTS )) && systemctl is-active --quiet nightingale; then
      log "stopping existing nightingale listener on ports 80/443 before restarting caddy"
      as_root systemctl stop nightingale || true
    fi
    caddy_validate_or_die
    as_root systemctl restart caddy
  else
    ok "caddy config unchanged; not restarting"
  fi

  as_root systemctl enable nightingale || true
  if (( NIGHTINGALE_DIRTY )) || ! systemctl is-active --quiet nightingale; then
    as_root systemctl restart nightingale
  else
    ok "nightingale unchanged; not restarting"
  fi
}

# Pull Caddy's local CA root cert via the admin API and publish it at
# /etc/caddy/nightingale-root.crt so devices that want HTTPS can bootstrap
# trust over plain HTTP. The admin API listens on 127.0.0.1:2019 by
# default and exposes /pki/ca/local with the cert as a JSON string -
# identical response across distros, no filesystem traversal of
# /var/lib/caddy.
publish_root_cert() {
  local tries=0 body=""
  while (( tries < 10 )); do
    body="$(curl -fsS --max-time 2 http://127.0.0.1:2019/pki/ca/local 2>/dev/null || true)"
    [[ -n "$body" ]] && break
    sleep 0.5
    tries=$((tries + 1))
  done

  if [[ -z "$body" ]]; then
    warn "couldn't reach Caddy admin API at 127.0.0.1:2019 after 5s - skipping root.crt publish"
    warn "Caddy may still be starting up; re-run this installer in a few seconds and the cert will be published, or check 'systemctl status caddy' for errors"
    return
  fi

  # The JSON `root_certificate` field is a single string with literal \n
  # escapes; one awk pass to grab it (RS=\0 so the regex sees the whole
  # body as one record), and a gsub to turn the JSON \n escapes back into
  # real newlines so the result is a valid PEM file.
  local pem pem_tmp
  pem="$(printf '%s' "$body" | awk '
    BEGIN { RS="\0" }
    match($0, /"root_certificate":"[^"]+"/) {
      s = substr($0, RSTART+20, RLENGTH-21)
      gsub(/\\n/, "\n", s)
      print s
    }
  ')"

  if [[ -z "$pem" ]] || ! grep -q 'BEGIN CERTIFICATE' <<<"$pem"; then
    warn "Caddy admin API didn't return a usable root_certificate field - skipping"
    return
  fi

  pem_tmp="$(mktemp)"
  printf '%s' "$pem" > "$pem_tmp"
  as_root install -m 0644 -o root -g root "$pem_tmp" "$ROOT_CRT_PATH"
  rm -f "$pem_tmp"
  ok "published trust root at $ROOT_CRT_PATH (served at http://<host>/root.crt)"
}

# ── Summary ────────────────────────────────────────────────────────────

print_summary() {
  local host="$HOSTNAME_LABEL"
  cat <<EOF

${C_BOLD}Nightingale is up.${C_RST}

  ${C_BOLD}Open${C_RST}   http://${host}     ${C_DIM}(browser will tag this "Not Secure" - that's expected on plain HTTP)${C_RST}
  ${C_BOLD}Logs${C_RST}   journalctl -u nightingale -f
  ${C_BOLD}Status${C_RST} systemctl status nightingale caddy

${C_BOLD}First launch:${C_RST} open http://${host} on any device, pick a data folder
in the setup wizard, then add your songs folder from the main menu.

${C_BOLD}Running a host firewall?${C_RST} open ${C_BOLD}80/tcp${C_RST}, ${C_BOLD}443/tcp${C_RST}, ${C_BOLD}5353/udp${C_RST} ${C_DIM}(mDNS needs UDP/5353 or .local won't resolve from other devices)${C_RST}
${C_DIM}  ufw:       sudo ufw allow 80/tcp && sudo ufw allow 443/tcp && sudo ufw allow 5353/udp${C_RST}
${C_DIM}  firewalld: sudo firewall-cmd --permanent --add-service={http,https,mdns} && sudo firewall-cmd --reload${C_RST}

${C_BOLD}For mic capture (and other secure-context APIs): install the trust root.${C_RST}
${C_DIM}Plain HTTP \`*.local\` is *not* a secure context for browsers, so${C_RST}
${C_DIM}\`navigator.mediaDevices.getUserMedia\` and friends are blocked there.${C_RST}
${C_DIM}You need HTTPS once - install the cert and use \`https://${host}\` from${C_RST}
${C_DIM}then on. (You may be able to "Continue Anyway" past the warning to use${C_RST}
${C_DIM}the rest of the app over HTTP, but mic stays disabled until trust is in.)${C_RST}

  ${C_DIM}# from any device on the LAN${C_RST}
  curl -O http://${host}/root.crt

  Then trust the cert per OS:
    ${C_DIM}macOS  ${C_RST}open root.crt   ${C_DIM}(Keychain Access -> Always Trust)${C_RST}
    ${C_DIM}Linux  ${C_RST}sudo cp root.crt /usr/local/share/ca-certificates/nightingale.crt && sudo update-ca-certificates
    ${C_DIM}iOS    ${C_RST}AirDrop root.crt, install profile, then Settings -> General -> About -> Certificate Trust Settings -> enable
    ${C_DIM}Android${C_RST}Settings -> Security -> Encryption & credentials -> Install from storage -> CA certificate
    ${C_DIM}Windows${C_RST}Double-click root.crt -> Install Certificate -> Local Machine -> Trusted Root Certification Authorities

${C_DIM}(neither the data folder nor the songs folder is baked into the install;${C_RST}
${C_DIM} both are picked in-app and persisted to config.json)${C_RST}

EOF
}

# ── Main ───────────────────────────────────────────────────────────────

build_steps() {
  local mode_render="${1:-resolved}"
  local user_d hostname_d datadir_d acquire_step
  user_d="$(plan_value     "$mode_render" NIGHTINGALE_USER     "$SERVICE_USER"   "<user>")"
  hostname_d="$(plan_value "$mode_render" NIGHTINGALE_HOSTNAME "$HOSTNAME_LABEL" "<host>.local")"
  datadir_d="$(plan_value  "$mode_render" NIGHTINGALE_DATA_DIR "$DATA_DIR"       "$DEFAULT_DATA_DIR")"

  if [[ "$MODE" == "source" ]]; then
    local source_d
    source_d="$(plan_value "$mode_render" NIGHTINGALE_SOURCE "$SOURCE_DIR" "<repo path>")"
    acquire_step="Build nightingale from source at ${source_d}"
  else
    local version_d repo_d
    version_d="$(plan_value "$mode_render" NIGHTINGALE_VERSION "$VERSION" "<version>")"
    repo_d="$(plan_value    "$mode_render" NIGHTINGALE_REPO    "$REPO"    "<github repo>")"
    acquire_step="Download nightingale ${version_d} binary from ${repo_d}"
  fi

  STEPS=(
    "Ensure caddy + avahi-daemon are installed"
    "Create system user '${user_d}' (if missing)"
    "Prepare data directory ${datadir_d}"
    "${acquire_step}"
    "Install systemd unit at ${UNIT_PATH}"
    "Install Caddy reverse-proxy config (additive if you already run Caddy)"
    "Install Avahi mDNS service advertisement"
    "Pin avahi to publish '${hostname_d}'"
    "Filter avahi mDNS to skip virtual bridges (docker / podman / virbr / lxc)"
    "Verify ports 80 and 443 are free"
    "Enable and start avahi-daemon, caddy, nightingale"
    "Publish Caddy's local CA root at ${ROOT_CRT_PATH}"
  )
}

main() {
  parse_args "$@"
  require_systemd
  require_privilege_tool

  # 1) Resolve every var from env overrides / defaults / unit introspection
  # so the plan we print is honest about what'll happen on Enter-through.
  # 2) Build + print the plan before prompts so the operator can Ctrl+C
  # without committing. 3) Run the prompts. 4) Re-build the plan so the
  # step headers below show the values actually picked.
  resolve_defaults
  build_steps preview
  print_banner
  print_plan
  configure
  build_steps

  local assets
  assets="$(resolve_assets_dir)"

  step; install_packages
  step; ensure_user
  step; ensure_data_dir
  step; acquire_binary
  step; drop_systemd_unit "$assets"
  step; drop_caddy_config "$assets"
  step; drop_avahi_service "$assets"
  step; configure_avahi_hostname
  step; configure_avahi_interfaces
  step; check_port_conflicts
  step; start_services
  step; publish_root_cert

  print_summary
}

main "$@"

