<p align="center">
  <img src="misc/images/icon.png" width="128" height="128" alt="Stemcraft icon">
</p>

<h1 align="center">Stemcraft</h1>

<p align="center">
  <b>English</b> · <a href="README.zh-CN.md">简体中文</a> · <a href="README.zh-TW.md">繁體中文</a>
</p>

Split a song into six stems — drums, bass, vocals, piano, other, guitar — then pick the ones
you want and export them as a new mix. Inspired by
[Haig012/guitar-extractor](https://github.com/Haig012/guitar-extractor), rewritten in Rust.

Both a CLI and a macOS desktop app are included. The desktop app separates a song (or a part
of it), lets you mute / solo / balance the six stems while listening, and exports the mix — and
optionally each stem and a chord chart — as WAV, FLAC, MP3, M4A (AAC) or OGG.

Separation runs Demucs `htdemucs_6s` on the GPU (Metal) through
[demucs-rs](https://github.com/nikhilunni/demucs-rs) / [Burn](https://burn.dev) — no Python,
PyTorch or ffmpeg. The GPUI ([gpui-kit](https://github.com/longbridge/gpui-kit)) desktop app
reuses the same core crate.

![The Stemcraft mixer in the dark theme](misc/images/screenshot-mixer.png)

## Layout

| Path | What |
|---|---|
| `crates/core` | Library: decoding (Symphonia), separation, guitar/backing mixdown, chord detection, weights cache |
| `crates/cli` | `stemcraft` command-line tool |
| `crates/app` | `stemcraft-app` desktop app (GPUI) |
| `misc/python_src` | Earlier Python prototype (demucs-mlx), kept for reference |

## Build

Requires Rust 1.98.1 (pinned in `rust-toolchain.toml`; rustup installs it automatically).

```bash
cargo build --release
```

## Desktop app

```bash
cargo run --release -p stemcraft-app      # run from source
script/bundle-macos.sh                    # → dist/Stemcraft.app and dist/Stemcraft-<version>.dmg
```

The bundled app carries the model weights in `Contents/Resources/`, so it works offline with
nothing else to install. It is ad-hoc signed, so on another Mac the first launch is blocked;
open System Settings → Privacy & Security and click "Open Anyway" (on macOS versions before 15,
right-click → Open also works). Distributing to others needs a Developer ID signature and
notarization.

Settings (Stemcraft → Settings…, ⌘,) cover language (English, Simplified Chinese or Traditional
Chinese; follows the system by default), appearance (follow system / light / dark),
the export location and defaults, the audio output device, the GPU optimization cache and
third-party licenses. They're saved to `~/Library/Application Support/Stemcraft/settings.json`.

## Command-line usage

```bash
stemcraft song.mp3                 # → output/song/song_guitar.wav, song_no_guitar.wav
stemcraft song.mp3 --chords        # + song_chords.lrc / .txt
stemcraft song.mp3 -r 1:30-3:00    # only process part of the song
stemcraft song.mp3 --all-stems     # + all six raw stems in output/song/stems/
```

(`cargo run --release -- song.mp3 …` works too.) Inputs: WAV, AIFF, FLAC, MP3, OGG Vorbis,
M4A (AAC/ALAC); mono or stereo. Outputs are 32-bit float WAV at the input's sample rate, so the
summed backing track never clips.

The first run downloads the model weights (~55 MB, Hugging Face) into
`~/Library/Caches/demucs-rs/` and compiles/autotunes GPU kernels (cached under
`~/Library/Application Support/autotune/`); later runs start immediately.

## Tests

```bash
cargo test
```
