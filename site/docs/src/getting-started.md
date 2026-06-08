# Getting Started

## Download

| Platform | Format                    | Architectures      |
| -------- | ------------------------- | ------------------- |
| Linux    | `.deb`, `.rpm` | x86_64, ARM (arm64) |
| macOS    | `.dmg`                    | Apple Silicon, Intel |
| Windows  | Installer `.exe`, `.msi` | x86_64 |

<br />

Download the latest version from the [Releases](https://github.com/rzru/nightingale/releases) page.

Supported audio formats: `.mp3`, `.flac`, `.ogg`, `.opus`, `.wav`, `.m4a`, `.aac`, `.wma`.

Supported video formats: `.mp4`, `.mkv`, `.avi`, `.webm`, `.mov`, `.m4v`.

UltraStar Deluxe songs (`.usdx`, plus `.txt` files whose contents look like USDX) are also picked up automatically and bypass the analyzer pipeline entirely. See [UltraStar Deluxe](./usdx.md) for the supported tags and folder layout.

## macOS: Removing the Quarantine Flag

macOS automatically adds a quarantine attribute to files downloaded from the internet. Since Nightingale is not signed with an Apple Developer ID, Gatekeeper will block it with a message like _"app is damaged and can't be opened"_ or _"Apple cannot check it for malicious software"_.

To fix this, remove the quarantine attribute after moving the Nightingale.app to Applications:

```bash
xattr -cr /Applications/Nightingale.app
```

This tells macOS to clear (`-c`) all extended attributes recursively (`-r`) from the app bundle, which removes the `com.apple.quarantine` flag that triggers Gatekeeper. The app itself is safe — it's just not code-signed.

## First Launch

On first launch, Nightingale will guide you through setup:

1. **Choose data folder** — select where cache, models, videos, vendor tools, and the library database are stored. After setup, Settings can split cache, videos, models, and vendor tools into separate folders.
2. **Downloads ffmpeg** — needed for audio/video processing
3. **Downloads uv** — Python package manager
4. **Installs Python 3.10** — via uv, isolated from your system Python
5. **Creates virtual environment** — with PyTorch, WhisperX, Demucs, and UVR models
6. **Downloads ML models** — stem separation and transcription models
7. **Pre-downloads video backgrounds** — Pixabay videos for the first session

This process takes a few minutes and shows a progress screen. After setup completes, Nightingale is ready to use.

<!-- TODO: screenshot of the setup/bootstrap progress screen -->

![Setup progress](images/setup.png)

## Adding Music

When prompted, select your music folder. Nightingale scans it for supported audio and video files. You can change this folder later from the sidebar actions menu.

## Analysis

Before a song can be played as karaoke, it needs to be analyzed:

1. Select a song from the library
2. Analysis runs automatically (stem separation → lyrics → transcription)
3. Results are cached — subsequent plays are instant

You can also batch analysis with **Analyze All** from the song list toolbar, or enable **Settings → Analysis → Auto-analyze** to queue newly discovered unanalyzed songs after each scan.

<!-- TODO: screenshot of the song library with a mix of analyzed/queued/not-analyzed songs -->

![Song library](images/library.png)

## Updating

On **macOS** and **Windows**, Nightingale checks for new releases once at launch. When one is available:

1. The sidebar avatar grows a small green dot.
2. The **Update** entry in the sidebar dropdown menu also shows the badge and opens a dialog with the version, release date, and notes.
3. Click **Install & Restart**. The signed bundle downloads (with a progress bar), installs, and the app relaunches.

Platform notes:

- **macOS**: the bundle is replaced in place and the app relaunches.
- **Windows**: the installer runs in `passive` mode — a small progress window appears, the app exits, and it comes back automatically once the install finishes.

### Linux

The Linux build ships **without** the in-app updater. The **Update** entry is still in the sidebar menu, but instead of fetching a bundle it opens a dialog that explains this and gives you an **Open GitHub Releases** button. Pick the `.deb` or `.rpm` for your distro from the [Releases](https://github.com/rzru/nightingale/releases) page and install it the usual way.

There is no update badge on the sidebar avatar on Linux.

If the macOS or Windows dialog reports an error, see [Troubleshooting → Updates](./troubleshooting.md#updates).

## Force Re-setup

If something goes wrong with setup or dependencies, open the sidebar actions menu and select **Re-run Setup**. If you only need to move cache/models/videos/vendor files, use Settings so Nightingale migrates existing contents and avoids stale paths.
