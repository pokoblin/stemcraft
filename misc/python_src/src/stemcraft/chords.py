"""
Chord detection on an isolated guitar stem by chroma template matching.

CQT chroma → fixed-length blocks → cosine match against the 24 major/minor
triads → energy gate ("N.C." over silence) → majority-vote smoothing →
run-length merge. Reports triad names only (no 7ths, sus, inversions).
"""
from __future__ import annotations

from collections import Counter
from pathlib import Path
from typing import NamedTuple

import librosa
import numpy as np

PITCHES = ("C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B")
NO_CHORD = "N.C."
ANALYSIS_SR = 22050
HOP = 512


class Chord(NamedTuple):
    start: float
    end: float
    label: str


def _templates() -> tuple[np.ndarray, list[str]]:
    # Root and third outweigh the fifth, which relative major/minor pairs share.
    weights = {"root": 1.2, "third": 1.0, "fifth": 0.8}
    rows, labels = [], []
    for i, name in enumerate(PITCHES):
        for third, suffix in ((4, ""), (3, "m")):
            row = np.zeros(12, dtype=np.float32)
            row[i] = weights["root"]
            row[(i + third) % 12] = weights["third"]
            row[(i + 7) % 12] = weights["fifth"]
            rows.append(row)
            labels.append(name + suffix)
    tem = np.array(rows)
    return tem / np.linalg.norm(tem, axis=1, keepdims=True), labels


_TEMPLATES, _LABELS = _templates()


def detect_chords(
    y: np.ndarray,
    sr: int,
    *,
    block_s: float = 0.25,
    smooth_blocks: int = 5,
    min_dur: float = 0.5,
    energy_gate: float = 0.08,
) -> list[Chord]:
    """Detect chords in mono or (channels, samples) audio ``y``."""
    if y.ndim == 2:
        y = y.mean(axis=0)
    if sr != ANALYSIS_SR:
        y = librosa.resample(y, orig_sr=sr, target_sr=ANALYSIS_SR)
    if y.size == 0:
        return []
    duration = y.size / ANALYSIS_SR

    rms = librosa.feature.rms(y=y, hop_length=HOP)[0]
    peak = float(rms.max())
    if peak <= 0:
        return [Chord(0.0, duration, NO_CHORD)]
    chroma = librosa.feature.chroma_cqt(y=y, sr=ANALYSIS_SR, hop_length=HOP)

    block = max(1, round(block_s * ANALYSIS_SR / HOP))
    starts = range(0, chroma.shape[1], block)
    labels = []
    for b in starts:
        vec = np.median(chroma[:, b:b + block], axis=1)
        norm = np.linalg.norm(vec)
        if np.median(rms[b:b + block]) < energy_gate * peak or norm < 1e-6:
            labels.append(NO_CHORD)
        else:
            labels.append(_LABELS[int(np.argmax(_TEMPLATES @ (vec / norm)))])

    half = smooth_blocks // 2
    smoothed = [
        Counter(labels[max(0, i - half):i + half + 1]).most_common(1)[0][0]
        for i in range(len(labels))
    ]

    def t(block_index: int) -> float:
        return min(duration, float(librosa.frames_to_time(block_index * block, sr=ANALYSIS_SR, hop_length=HOP)))

    segments: list[Chord] = []
    run_start = 0
    for i in range(1, len(smoothed) + 1):
        if i == len(smoothed) or smoothed[i] != smoothed[run_start]:
            end = duration if i == len(smoothed) else t(i)
            segments.append(Chord(t(run_start), end, smoothed[run_start]))
            run_start = i
    return _absorb_short(segments, min_dur)


def _absorb_short(segments: list[Chord], min_dur: float) -> list[Chord]:
    """Fold segments shorter than ``min_dur`` into their predecessor, then re-merge runs."""
    out: list[Chord] = []
    for seg in segments:
        if out and (seg.end - seg.start < min_dur or seg.label == out[-1].label):
            out[-1] = out[-1]._replace(end=seg.end)
        else:
            out.append(seg)
    return out


def _clock(seconds: float) -> str:
    m, s = divmod(int(seconds), 60)
    return f"{m}:{s:02d}"


def write_lrc(chords: list[Chord], path: str | Path) -> Path:
    path = Path(path)
    lines = []
    for c in chords:
        m, s = divmod(c.start, 60)
        lines.append(f"[{int(m):02d}:{s:05.2f}]{c.label}")
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return path


def write_txt(chords: list[Chord], path: str | Path, title: str = "") -> Path:
    path = Path(path)
    lines = [f"# Chords — {title}".rstrip(" —"), ""]
    lines += [f"{_clock(c.start):>7}   {c.label}" for c in chords]
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return path
