# Library Sources

Nightingale can build your karaoke library from a local folder, Jellyfin, or Navidrome. Use the **Library** buttons in the sidebar to choose one.

Only one source is active at a time. You can switch later without losing already-analyzed songs that point to the same audio.

| Source | Best for | Supports video? | What you enter |
|---|---|---:|---|
| Folder | Music stored on this computer or drive | Yes | Folder path |
| Jellyfin | Music and videos on a Jellyfin server | Yes | Server URL, username, password |
| Navidrome | Music on Navidrome or another Subsonic server | No | Server URL, username, password |

## Local folder

Choose a music folder and Nightingale scans it recursively.

Use this when your files are already on this computer, an external drive, or a mounted network share. Folder libraries support audio files, video files, and UltraStar Deluxe song folders.

To update the list after adding or removing files, rescan from the sidebar. Existing analysis is reused when the file has not changed.

See [Getting Started](./getting-started.md#adding-music) for supported formats.

## Jellyfin

Use Jellyfin when your music or music videos live on a Jellyfin server.

1. Click the Jellyfin button in the **Library** sidebar section.
2. Enter your server URL, username, and password.
3. Click **Test connection**.
4. Click **Connect**.

After connecting, Nightingale lists songs and videos from Jellyfin in your library. Cover art loads as needed. When you analyze or play a song that needs processing, Nightingale downloads the original media once into its local cache. Later analyses reuse that cached copy.

Connection status appears on the Jellyfin button:

- Green: server reachable.
- Amber: last check failed. Hover for details.
- Grey: still checking.

## Navidrome / Subsonic

Use Navidrome when your music lives on a Navidrome or Subsonic-compatible server.

1. Click the Navidrome button in the **Library** sidebar section.
2. Enter your server URL, username, and password.
3. Click **Test connection**.
4. Click **Connect**.

Nightingale scans albums and songs from the server. Audio downloads only when a song is first analyzed, then stays in the local cache for reuse.

Navidrome sources are audio-only. Video items are not imported.

## Switching sources

Connect a different source whenever you want to change libraries. Nightingale rescans and shows songs from the new source.

Your analysis cache stays on disk. If you return to a source later, songs with the same audio can reuse existing stems, lyrics, and other analysis files.

## Passwords and tokens

Jellyfin and Navidrome credentials are saved so Nightingale can reconnect next time.

Credentials are encrypted in `config.json`, but you still should not share that file. If you previously used an older build with plain-text credentials, Nightingale wraps them the next time it saves settings.
