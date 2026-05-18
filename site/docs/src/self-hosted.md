# Self-Hosted Web Mode

Run Nightingale as a service on a Linux box on your LAN. Phones, laptops, tablets and TVs all open it in a browser. The default `http://<host>.local` URL works for everything except secure-context-only browser APIs (mic capture, clipboard, fullscreen) — to flip those on, install the trust root once per device and use `https://<host>.local` (one-time per device, instructions below). One install script, one URL.

The whole thing is one shell script (`scripts/install.sh`) that drops a systemd unit, a Caddy front-door, and an avahi advertisement onto a Linux host. Like Jellyfin, you pick the data folder and the songs folder from inside the app once the service is up — nothing is baked into the install.

## What you get

```text
LAN device ──http──► caddy.service ──► nightingale.service ──► /var/lib/nightingale
   │                    │   ▲
   │                    │   └── /root.crt  (served from /etc/caddy/, only matters if you want HTTPS)
   └── mDNS ───────► <hostname>.local

  (https on :443 is wired up too, opt-in — install root.crt on the device first)
```

- A `nightingale.service` running the web server on `127.0.0.1:8080`.
- A `caddy.service` fronting it on `:80` (works as-is, browser shows "Not Secure" — fine for the bulk of the app) and `:443` (HTTPS via Caddy's local CA — needed for mic capture and other secure-context-only browser APIs).
- An `avahi-daemon` advertisement so the box is reachable as `<hostname>.local` on the LAN — no DNS configuration on any device.

Open `http://<hostname>.local` to land in the app. To enable mic capture, install the LAN root cert (one-time per device, see below) and switch to `https://<hostname>.local`.

## Requirements

- A Linux host with `systemd`.
- `caddy` and `avahi-daemon` available. The installer is **distro-agnostic**: it probes for a package manager (`apt-get` / `dnf` / `pacman` / `zypper` / `apk`) and uses whichever one it finds. On distros without a recognised manager (NixOS, exotic source-builds), pre-install `caddy` + `avahi-daemon` yourself and the installer will skip the package step entirely.
- Root access on the host (the installer drops files under `/etc` and `/usr/local/bin`).
- Outbound HTTPS to GitHub (to fetch the binary and the vendor dependencies the app downloads on first launch).
- Optional but recommended: NVIDIA GPU + driver for faster stem separation and transcription. The Rust binary detects this at runtime; no separate CUDA install path.

## One-command install

```bash
curl -fsSL https://raw.githubusercontent.com/rzru/nightingale/main/scripts/install.sh | sudo bash
```

This downloads a prebuilt binary from the latest GitHub Release. The installer is **idempotent** — re-running upgrades the binary (it skips the download when you're already on the requested version) and restarts the services in place.

When it finishes the banner tells you to open `http://<hostname>.local`. That's it — you can ignore the rest of this page if you don't want HTTPS, don't have a pre-existing Caddy/avahi setup, and don't need to change defaults.

Env overrides for power users (every prompt accepts the matching variable and skips when set):

| Variable | Default | Purpose |
| --- | --- | --- |
| `NIGHTINGALE_VERSION` | `latest` | Tag on GitHub Releases to install (e.g. `v0.6.0`). |
| `NIGHTINGALE_REPO` | `rzru/nightingale` | Source repo for the binary tarball. |
| `NIGHTINGALE_HOSTNAME` | `$(hostname -s).local` | mDNS name to publish on the LAN. The installer writes `host-name=<label>` into `/etc/avahi/avahi-daemon.conf` (with backup) when the requested label differs from the system hostname — *unless* avahi already publishes an unrelated override (see [Co-existing with your own Caddy / avahi](#co-existing-with-your-own-caddy--avahi)). |
| `NIGHTINGALE_USER` | `nightingale` | System user the service runs as. |
| `NIGHTINGALE_DATA_DIR` | `/var/lib/nightingale` | Bootstrap dir for `config.json` and the published trust-root cert. Rendered into the systemd unit's `NIGHTINGALE_DATA_PATH` / `ReadWritePaths=` and the Caddyfile's `root` directive in lock-step. Re-runs over an existing install pick up whatever path that install's unit already advertises (so legacy hosts at `/home/<user>/.nightingale` keep their data on upgrade). Not your songs/library folder — that lives wherever you point the in-app setup wizard. |
| `NIGHTINGALE_FORCE_AVAHI_HOSTNAME` | unset | Set to `1` to overwrite an existing `host-name=` override in `/etc/avahi/avahi-daemon.conf`. Leave unset to preserve whatever's already there. |
| `NIGHTINGALE_FORCE_CADDYFILE` | unset | Set to `1` to overwrite `/etc/caddy/Caddyfile` even when it carries content we don't recognise (your own site config). The file is still backed up to `*.nightingale.bak` first. Without this, the installer aborts with an actionable error rather than letting Caddy fail with `ambiguous site definition` at startup. |

## Build from source

When there's no release yet, or you want to test local changes, build the server from a clone instead. Same script, same end state (same systemd unit, same Caddy config, same avahi advertisement) — the only difference is the binary is compiled locally:

```bash
git clone https://github.com/rzru/nightingale.git
cd nightingale
sudo bash scripts/install.sh --from-source
```

Running `scripts/install.sh` from inside a clone auto-detects the checkout and switches to source mode automatically; passing `--from-source` makes that explicit.

This needs `cargo`, `node`, and `pnpm` on the **invoking user's** login shell — `rustup`, `fnm`, `mise`, `nvm`, `asdf` are all fine; the installer re-execs the build through `sudo -iu $SUDO_USER -- bash -lc` so your shell's init loads.

If the script lives outside its repo (e.g. you copied it elsewhere), point it at the checkout with `--from-source=/path/to/nightingale` or `NIGHTINGALE_SOURCE=/path/to/nightingale`. Every override from the release installer applies here too.

## First launch

1. Open `http://<hostname>.local` on **any** device on the LAN.
2. The setup wizard opens. Pick a data folder — choose somewhere with enough disk (videos, ML models, and stem caches live here).
3. Vendor dependencies download (`ffmpeg`, `uv`, Python 3.10, PyTorch, WhisperX, Demucs, UVR models). Progress bar in the wizard.
4. After setup, open the sidebar's library actions menu, click **Choose folder**, and type the absolute path to your songs (e.g. `/srv/music/karaoke`). The library scans.

Neither the data folder nor the songs folder is configured by the installer — both are picked in the app, persisted to `config.json` in the installer's bootstrap path (default `/var/lib/nightingale`, overridable via `NIGHTINGALE_DATA_DIR`), and survive upgrades.

## What works over plain HTTP, and what needs HTTPS

Most of the app — browsing the library, playing songs, queuing, scoring — runs fine on `http://<hostname>.local`. Browsers will tag the page **"Not Secure"** in the address bar, which is expected: per the [W3C Secure Contexts](https://w3c.github.io/webappsec-secure-contexts/) spec, only `localhost` / `*.localhost` / loopback IPs / `https://` / `wss://` / `file://` count as potentially trustworthy origins. `*.local` mDNS hostnames are **not** on that list — they're still treated as a regular insecure HTTP origin.

What that "Not Secure" tag actually breaks: the secure-context-only browser APIs the app uses for mic capture (`navigator.mediaDevices.getUserMedia`), the clipboard, fullscreen, and a few other gated features. On plain HTTP `<hostname>.local`:

- **Chromium / Edge** — `navigator.mediaDevices` is `undefined`. Mic doesn't work, period. You may be able to "Continue Anyway" past mixed-content warnings to use the rest of the app, but mic stays off until the origin is HTTPS.
- **Firefox** — same default; `getUserMedia` rejects the promise. Some about:config flips can override this on a per-origin basis, but it's not something you should ask every visitor to do.
- **Safari (iOS / macOS)** — strictest of all; the prompt won't even appear.

The reliable fix is the **opt-in HTTPS path** below: install the LAN root cert once per device, then use `https://<hostname>.local`. Caddy's `:443` listener is already up and minting certs from its local CA — the only missing piece is each device trusting that CA.

## Opt in to HTTPS (mic-capable, green padlock)

The installer publishes Caddy's local-CA root cert at a plain-HTTP path so devices can fetch it before they trust it:

```bash
# From any device on the LAN:
curl -O http://<hostname>.local/root.crt
```

Per OS:

- **macOS** — `open root.crt`, then in Keychain Access right-click the Nightingale cert → **Get Info** → **Trust** → **Always Trust**.
- **Linux** — `sudo cp root.crt /usr/local/share/ca-certificates/nightingale.crt && sudo update-ca-certificates`. Firefox keeps its own store; add it via *Preferences → Privacy & Security → Certificates → View Certificates → Authorities → Import*.
- **iOS / iPadOS** — AirDrop `root.crt` to the device (or email it to yourself), tap to install the profile, then **Settings → General → About → Certificate Trust Settings** and flip the toggle for the Nightingale CA.
- **Android** — **Settings → Security → Encryption & credentials → Install a certificate → CA certificate**, point at the downloaded file. Some browsers (Firefox) keep a separate store and need a second import there.
- **Windows** — Double-click `root.crt` → **Install Certificate** → **Local Machine** → **Place all certificates in the following store** → **Trusted Root Certification Authorities**.

After this, the device treats `https://<hostname>.local` as fully secure forever (or until you reinstall the box and the local CA is regenerated).

## Co-existing with your own Caddy / avahi

The installer is designed to drop in next to whatever you already run, not replace it. **Re-runs are fully idempotent** — nothing accumulates or appends on every run. Concretely:

- **Packages.** If `caddy` and `avahi-daemon` are already installed (regardless of how — distro package, NixOS, custom build), the installer detects them and skips the package-manager step entirely. Otherwise it probes for `apt-get` / `dnf` / `pacman` / `zypper` / `apk` and uses the first one it finds.
- **Caddy snippet.** Whether the installer writes the main `/etc/caddy/Caddyfile` itself or drops a snippet at `/etc/caddy/Caddyfile.d/nightingale.caddy`, the file is **overwritten** with the same content on every run (via `install(1)`, atomic replace) — never appended to.
- **Caddy main file** (your hand-written one). When `/etc/caddy/Caddyfile` already has site config you wrote, the installer leaves it alone, drops the nightingale snippet at `/etc/caddy/Caddyfile.d/nightingale.caddy`, and appends a single `import Caddyfile.d/*.caddy` line to your main Caddyfile. The append is gated on a grep — the second run finds the existing import and does nothing. The snippet has no global options block (it uses per-site `tls internal { on_demand }` instead of relying on global `local_certs`) so it imports cleanly into any position in your Caddyfile. The snippet defines `http://` and `:443` listeners — if your own config already binds either (either in the main Caddyfile **or** in any other `Caddyfile.d/*.caddy` snippet), the installer **aborts with a clear error before starting Caddy** (so you never see Caddy's opaque `ambiguous site definition` failure in `journalctl`). You then have two options: (a) move your conflicting listeners into their own file under `Caddyfile.d/` (caddy will pick them up via the same import line) and re-run; or (b) re-run with `NIGHTINGALE_FORCE_CADDYFILE=1` to let the installer take over the main Caddyfile (your original is backed up to `*.nightingale.bak`). As a final safety net, the installer runs `caddy validate` on the merged config before restarting — if the merge somehow doesn't parse, your previously-running caddy keeps running and the installer aborts with the actual validation error.
- **Caddy main file** (distro template or fresh box). The Arch / Debian / Fedora `caddy` packages ship an example Caddyfile by default. The installer asks the active package manager (`dpkg -V` / `pacman -Qkk` / `rpm -V`) whether `/etc/caddy/Caddyfile` is unmodified from what the package shipped — if so, it's treated as "operator never asked for this content" and is overwritten with our config plus a `# managed-by: nightingale-installer` header at the top. On re-runs the installer only overwrites files that still carry that header; remove the header and the file is yours. The installer no longer relies on string-matching the distro template's banner comment, so localized or reworded distro templates are detected correctly.
- **Avahi service file.** Goes in `/etc/avahi/services/nightingale.service`, written with `install(1)`. Naturally additive against your other service files (they live as separate files in the same directory), and re-runs overwrite the same bytes — no duplicates.
- **Avahi hostname (`host-name=`).** The installer reads the current `host-name=` value before touching anything. If it already matches what you asked for, the function returns early — zero edits, zero appends. Otherwise it does an **in-place `sed` replacement** of the existing line. The only fallback path that uses `>>` (append) is for a genuinely empty `avahi-daemon.conf` with no `[server]` section *and* no `host-name=` line anywhere; after that first run the line is present, so the next run hits the in-place replacement branch instead. If avahi already publishes an *unrelated* name (set by you or another package), the installer leaves it alone and warns that mDNS will not resolve `<hostname>.local`. Set `NIGHTINGALE_FORCE_AVAHI_HOSTNAME=1` to override.
- **Avahi virtual-bridge filter (`deny-interfaces=`).** Avahi publishes on *every* interface that's up by default — including `docker0`, `br-<id>`, `podman*`, `virbr0`, `lxcbr*`. Those bridges are host-internal and not routable from the LAN, but `getent hosts <host>.local` happily returns their IPs, so `curl <host>.local` can pick a dead bridge address and hang on TCP timeout. The installer enumerates the present interfaces via `ip link`, filters to the well-known virtual-bridge name patterns, and merges them into `deny-interfaces=` (preserving any entries you added yourself). Tunnel interfaces (`tailscale*`, `wg*`, `tun*`, `tap*`) are deliberately **not** filtered — those usually *are* the path you want nightingale reachable on.
- **Backups (`*.nightingale.bak`).** Created exactly once per file, the first time the installer touches it. Subsequent runs never overwrite or recreate the backup — your original pre-install state stays preserved.
- **Ports 80 and 443.** Pre-flight check before restarting caddy: if anything other than caddy is listening, the installer aborts with the conflicting process listed. Stop it (or move it off the standard HTTP/HTTPS ports) and re-run. The check matches the listener's program name exactly (`caddy`), not as a substring — so a process whose argv happens to contain `caddy` doesn't get a false pass.
- **Service restarts.** `caddy` and `avahi-daemon` are restarted **only** when the installer actually changed their config (`Caddyfile` rewrite, `host-name=` edit, `deny-interfaces=` change, etc.). A re-run on a healthy host where nothing changed leaves the daemons running untouched — your in-flight Caddy connections and other consumers' mDNS state aren't bounced for nothing. As a safety net, if a service is found stopped, the installer brings it back up regardless.
- **APT sources (Debian / Ubuntu).** When `caddy` is already in your distro's repos, the installer installs from there. The Cloudsmith upstream apt source only gets added when the distro genuinely doesn't ship caddy (older Debian, very minimal images) — never as a one-way upgrade of an already-installed distro caddy.

If you ever want to verify a re-run truly didn't change anything, `sudo wc -l /etc/caddy/Caddyfile /etc/avahi/avahi-daemon.conf` before and after should report the same line counts (and `diff` should be empty).

## Firewall

If your host runs a firewall (`ufw`, `firewalld`, raw `nftables`/`iptables`), nightingale needs three ports open **inbound** from your LAN. The installer never touches your firewall — that's a deliberate operator decision.

| Port   | Proto | What for                                                                                              |
| ------ | ----- | ----------------------------------------------------------------------------------------------------- |
| `80`   | TCP   | Caddy serving the app over HTTP — works for everything except secure-context-only APIs (mic, clipboard, fullscreen). |
| `443`  | TCP   | Caddy serving HTTPS — needed for mic capture and the rest of the secure-context-only APIs. Open it. |
| `5353` | UDP   | Avahi mDNS — **without this `<hostname>.local` will not resolve from other devices on the LAN.**      |

**ufw** (Debian / Ubuntu / Raspberry Pi OS):

```bash
sudo ufw allow 80/tcp   comment 'nightingale http'
sudo ufw allow 443/tcp  comment 'nightingale https (optional)'
sudo ufw allow 5353/udp comment 'nightingale mDNS'
sudo ufw reload
```

**firewalld** (Fedora / RHEL / openSUSE):

```bash
sudo firewall-cmd --permanent --add-service=http
sudo firewall-cmd --permanent --add-service=https
sudo firewall-cmd --permanent --add-service=mdns
sudo firewall-cmd --reload
```

If `ufw status` says `Status: inactive` (or `systemctl is-active firewalld` says `inactive`), there's nothing to open — the host already accepts inbound LAN traffic by default and the installer's `<hostname>.local` URL will just work.

## Permissions for your songs folder

The service runs as the unprivileged `nightingale` system user, so the songs folder you pick has to be readable by it. If your songs live in a path that isn't world-readable, give the service user access:

```bash
sudo setfacl -m u:nightingale:rx /srv/music
sudo setfacl -R -m u:nightingale:rx /srv/music/karaoke
```

Or grant via group membership — whatever fits your host's setup. The error message in the app's library view will tell you if the scan can't enter a directory.

`NIGHTINGALE_DATA_DIR` can be anything the service user can read and write (including a `0700` home directory) — Caddy never touches it directly. The trust root that `http://<host>/root.crt` returns is published to `/etc/caddy/nightingale-root.crt`, where the `caddy` system user always has read access regardless of how `DATA_DIR` is configured.

## Day-to-day operation

```bash
systemctl status nightingale caddy        # service health
journalctl -u nightingale -f              # tail server logs
journalctl -u caddy -f                    # tail TLS / proxy logs
sudo systemctl restart nightingale        # restart after editing config.json by hand
```

The Rust server's config lives at `<data-dir>/config.json` and is rewritten any time you change settings in the app.

## Changing the hostname or LAN IP

The Caddyfile at `/etc/caddy/Caddyfile` listens on `:80` and `:443` for every hostname/IP the request arrives on, so you don't need to edit it when you move the box.

The mDNS name comes from avahi. The simplest way to change it is to re-run the installer with the new value:

```bash
sudo NIGHTINGALE_HOSTNAME=nightingale.local bash scripts/install.sh
```

This patches `/etc/avahi/avahi-daemon.conf` to set `host-name=nightingale` (backing the original config up to `avahi-daemon.conf.nightingale.bak`) and restarts `avahi-daemon`. The new `<label>.local` propagates over the LAN within a few seconds. Setting `NIGHTINGALE_HOSTNAME` back to your system's `hostname -s` clears the override so avahi falls back to the system name. If avahi already publishes an unrelated override that the installer wouldn't touch by default, set `NIGHTINGALE_FORCE_AVAHI_HOSTNAME=1` to force it.

If you'd rather rename the box itself, `hostnamectl set-hostname <new>` followed by `sudo systemctl restart avahi-daemon` also works and avoids the avahi override.

If you want a stable LAN IP, set a DHCP reservation on your router and (optionally) add a static entry on devices that don't speak mDNS:

```text
192.168.1.10  nightingale.local
```

## Upgrading

```bash
curl -fsSL https://raw.githubusercontent.com/rzru/nightingale/main/scripts/install.sh | sudo bash
```

The installer redownloads the binary (skipping the download when you're already on the requested version), replaces `/usr/local/bin/nightingale`, and restarts the service. Your data folder, config, library database, and cache are untouched.

To pin a version, pass `NIGHTINGALE_VERSION=v0.6.0`.

## Uninstall

```bash
sudo systemctl disable --now nightingale
sudo rm /etc/systemd/system/nightingale.service
sudo systemctl daemon-reload
sudo rm -f /usr/local/bin/nightingale /usr/local/bin/nightingale.etag /usr/local/bin/nightingale.version
sudo rm /etc/avahi/services/nightingale.service && sudo systemctl restart avahi-daemon

# When the installer wrote our own Caddyfile (no pre-existing config), it's
# safe to remove. When it added an import line to your Caddyfile, just
# delete /etc/caddy/Caddyfile.d/nightingale.caddy and the import directive.
# sudo rm /etc/caddy/Caddyfile.d/nightingale.caddy && sudo systemctl reload caddy

# Data is preserved by default. Remove if you really want it gone:
# sudo rm -rf /var/lib/nightingale   # or wherever NIGHTINGALE_DATA_DIR pointed
# sudo userdel nightingale
```
