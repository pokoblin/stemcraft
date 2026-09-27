# Stemcraft — Python prototype

Python reference implementation (superseded by the Rust version at the repo root). Command-line MVP inspired by [Haig012/guitar-extractor](https://github.com/Haig012/guitar-extractor):
split a song into an **isolated guitar** track and a **guitar-free backing** track, optionally with a
timed chord sheet. Separation runs Demucs `htdemucs_6s` natively on Apple Silicon through
[demucs-mlx](https://github.com/ssmall256/demucs-mlx) — no PyTorch or ffmpeg at runtime.

## Setup

```bash
uv sync
```

Convert the model weights once (downloads the official Demucs checkpoint, ~100 MB, into
`~/.cache/demucs-mlx`). This needs PyTorch, so it runs in a throwaway environment:

```bash
uv run --isolated --no-project --python 3.12 --with 'demucs-mlx[convert]==1.4.14' python scripts/convert_weights.py
```

`scripts/convert_weights.py` works around a demucs-mlx 1.4.14 bug where the checkpoint validator
rejects `htdemucs_6s` (integer keys in its training metadata).

## Usage

```bash
uv run stemcraft song.mp3                 # → output/song/song_guitar.wav, song_no_guitar.wav
uv run stemcraft song.mp3 --chords        # + song_chords.lrc / .txt
uv run stemcraft song.mp3 -r 1:30-3:00    # only process part of the song
uv run stemcraft song.mp3 --all-stems     # + all six raw stems in output/song/stems/
```

Outputs are 32-bit float WAV at 44.1 kHz, so the summed backing track never clips.

## Tests

```bash
uv run pytest
```
