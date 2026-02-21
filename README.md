# SONE

An unofficial native Linux desktop client for [Tidal](https://tidal.com) music streaming.

**A valid Tidal account and subscription is required.** SONE does not provide access to any content — it connects to Tidal's service using your own credentials, the same way the official apps do on other platforms. This project exists because Tidal does not offer an official desktop client for Linux.

## Features

- Stream music from your Tidal library in up to lossless quality
- Browse, search, and manage your playlists, albums, and artists
- Queue management with playback history
- Volume normalization (ReplayGain) with album/track mode
- Exclusive output mode (ALSA) for dedicated audio devices
- Bit-perfect playback for audiophile setups
- MPRIS integration (media keys, DE widgets, taskbar controls)
- System tray with playback controls
- Keyboard shortcuts
- Encrypted local storage for credentials and cache
- Persistent sessions across restarts

## Prerequisites

**Rust:**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
```

**Node.js** 18+ (via nvm, fnm, or your preferred method)

**System dependencies** (Ubuntu/Debian):

```bash
sudo apt install -y \
    build-essential curl wget file patchelf \
    libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev \
    libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
    gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-libav \
    libsecret-1-dev
```

Optional (for exclusive ALSA output):

```bash
sudo apt install -y gstreamer1.0-alsa
```

For other distros, install the equivalent packages (e.g. `gst-plugins-*` on Arch, `gstreamer1-plugins-*` on Fedora).

## Build & Run

```bash
git clone https://github.com/lullabyX/sone.git
cd sone
npm install
npm run tauri dev
```

## Usage

1. Launch the app and click **Login with Tidal**
2. Enter the displayed code at [link.tidal.com](https://link.tidal.com)
3. Your library loads automatically — browse and play

## Tech Stack

- **Backend:** Rust (Tauri 2)
- **Frontend:** React 19, Tailwind 4, Jotai
- **Audio:** GStreamer
- **Config:** `~/.config/sone/`

## Disclaimer

SONE is an independent, community-driven project. It is **not affiliated with, endorsed by, or connected to Tidal** in any way. All content is streamed directly from Tidal's service and requires a valid paid subscription. SONE does not download, redistribute, or circumvent protection of any content.

All trademarks belong to their respective owners.

## License

[GPL-3.0-only](LICENSE)
