# Stemcraft

Split a song into six stems — drums, bass, vocals, piano, other, guitar — then pick the ones
you want and export them as a new mix. Inspired by
[Haig012/guitar-extractor](https://github.com/Haig012/guitar-extractor), rewritten in Rust.

The CLI currently produces an **isolated guitar** track and a **guitar-free backing** track
(plus all raw stems and an optional timed chord sheet); free stem selection comes with the
desktop app.

Separation runs Demucs `htdemucs_6s` on the GPU (Metal) through
[demucs-rs](https://github.com/nikhilunni/demucs-rs) / [Burn](https://burn.dev) — no Python,
PyTorch or ffmpeg. A GPUI ([gpui-kit](https://github.com/longbridge/gpui-kit)) desktop app will
reuse the same core crate.

## Layout

| Path | What |
|---|---|
| `crates/core` | Library: decoding (Symphonia), separation, guitar/backing mixdown, chord detection, weights cache |
| `crates/cli` | `stemcraft` command-line tool |
| `misc/python_src` | Earlier Python prototype (demucs-mlx), kept for reference |

## Build

Requires Rust 1.98.1 (pinned in `rust-toolchain.toml`; rustup installs it automatically).

```bash
cargo build --release
```

## Usage

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
