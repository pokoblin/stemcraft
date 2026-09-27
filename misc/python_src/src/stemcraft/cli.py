"""Command-line entry point: ``stemcraft song.mp3``."""
from __future__ import annotations

import argparse
import re
import sys
import time
from pathlib import Path

from . import chords as chord_mod
from .separation import (
    DEFAULT_MODEL,
    ModelUnavailableError,
    load_audio,
    load_separator,
    separate,
    split_guitar,
    write_wav,
)
from .timerange import parse_range


def _safe_name(stem: str) -> str:
    # macOS filenames are Unicode, so keep non-ASCII titles; drop only path separators.
    return re.sub(r'[/:\x00-\x1f]', "_", stem).strip(". ") or "output"


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        prog="stemcraft",
        description="Split a song into an isolated guitar track and a guitar-free backing track "
        "(Demucs htdemucs_6s on Apple Silicon via MLX).",
    )
    p.add_argument("input", type=Path, help="audio file (wav, mp3, m4a, flac, ogg, …)")
    p.add_argument("-o", "--out", type=Path, default=Path("output"),
                   help="output root; each song gets its own subfolder (default: ./output)")
    p.add_argument("-r", "--range", dest="time_range", metavar="START-END",
                   help="process only part of the song, e.g. 1:30-3:00, 90-, -2:00")
    p.add_argument("-c", "--chords", action="store_true",
                   help="detect chords from the guitar stem (.lrc and .txt)")
    p.add_argument("--all-stems", action="store_true",
                   help="also write every raw stem (drums, bass, vocals, piano, other, guitar)")
    p.add_argument("-m", "--model", default=DEFAULT_MODEL, help=argparse.SUPPRESS)
    return p


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    src: Path = args.input
    if not src.is_file():
        print(f"error: input file not found: {src}", file=sys.stderr)
        return 2
    try:
        start, end = parse_range(args.time_range) if args.time_range else (0.0, None)
    except ValueError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2

    song = _safe_name(src.stem)
    out_dir = args.out / song
    out_dir.mkdir(parents=True, exist_ok=True)
    t0 = time.perf_counter()

    try:
        separator = load_separator(args.model)
    except ModelUnavailableError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    sr = separator.samplerate

    try:
        audio = load_audio(src, sr, start, end)
    except Exception as exc:
        print(f"error: could not decode {src}: {exc}", file=sys.stderr)
        return 1
    print(f"🎵 {src.name}  ({audio.shape[1] / sr:.1f}s @ {sr} Hz)")

    print(f"🎸 Separating with {args.model} …")
    t_sep = time.perf_counter()
    stems = separate(separator, audio)
    print(f"   done in {time.perf_counter() - t_sep:.1f}s")

    guitar, backing = split_guitar(stems)
    written = [
        write_wav(out_dir / f"{song}_guitar.wav", guitar, sr),
        write_wav(out_dir / f"{song}_no_guitar.wav", backing, sr),
    ]
    if args.all_stems:
        stem_dir = out_dir / "stems"
        stem_dir.mkdir(exist_ok=True)
        written += [write_wav(stem_dir / f"{song}_{name}.wav", a, sr) for name, a in stems.items()]

    if args.chords:
        print("🎼 Detecting chords …")
        found = chord_mod.detect_chords(guitar, sr)
        written.append(chord_mod.write_lrc(found, out_dir / f"{song}_chords.lrc"))
        written.append(chord_mod.write_txt(found, out_dir / f"{song}_chords.txt", song))
        named = sum(1 for c in found if c.label != chord_mod.NO_CHORD)
        print(f"   {named} chord segments")

    print(f"✅ Finished in {time.perf_counter() - t0:.1f}s → {out_dir}")
    for path in written:
        print(f"   {path.relative_to(out_dir)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
