# Aareco

A voice recorder organised in a topic tree, with marks, playback, effects, and
backups.

## What it does

- Records from any input device to mono 16-bit WAV at the device or a chosen
  sample rate.
- Keeps a crash-safe `.part` file while recording and finalises it on stop, so
  an interrupted session is recovered on the next start.
- Builds a nested topic tree. Deleting a topic moves its recordings to the
  parent instead of deleting audio.
- Adds marks while recording or during playback, nudges them, and jumps between
  them.
- Plays recordings with variable speed, seek, scrub, and per-recording volume.
- Applies VST3 or CLAP effects to a recording, keeping the original audio so the
  result can be reverted or re-rendered with different settings.
- Backs the whole library up to a folder and restores it, keeping the audio
  files in place.
- Ships a compact layout with a bottom navigation bar and a wide layout with a
  sidebar.

## Data

Desktop data lives in `~/.local/share/aareco`:

```
library.json        recordings, topics, marks, metadata
settings.json       device, theme, playback and recording preferences
recordings/         WAV files, including rendered effect versions
plugins/            imported CLAP plugins
```

On Android the same layout is rooted at the app's internal data directory (will switch to game-utils crate later)

## Build and run

Requires Rust 1.98 and the platform audio libraries (ALSA on Linux).

```sh
cargo run --release --bin aareco-desktop
```

Recordings are captured through `cpal`, playback through `rodio`, effects run
through Yadaw's plugin host, and the UI is drawn with
[Repose](https://github.com/repolevedavaj/repose).

The crate split keeps capture and storage independent from the UI:

- `crates/recorder`: device capture, ring buffer, peaks, WAV writing, recovery.
- `crates/store`: library model, topic tree, marks, search, backup/restore.
- `crates/fx`: plugin hosting, effect chains, and offline rendering.

## Android

The package name is `app.aareco` and the app requests `RECORD_AUDIO` on first
launch. Packaging resources, including the adaptive icon, are in
`others/packaging`.

## License

AGPL-3.0-or-later. See `LICENSE`.
