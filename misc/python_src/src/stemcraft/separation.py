"""Stem separation with demucs-mlx (htdemucs_6s) and guitar / backing mixdown."""
from __future__ import annotations

from pathlib import Path

import numpy as np
import soundfile as sf

DEFAULT_MODEL = "htdemucs_6s"
GUITAR = "guitar"


class ModelUnavailableError(RuntimeError):
    pass


def load_separator(model: str = DEFAULT_MODEL, progress: bool = True):
    from demucs_mlx import Separator

    try:
        return Separator(model=model, progress=progress)
    except Exception as exc:
        raise ModelUnavailableError(
            f"Could not load MLX weights for {model!r}: {exc}\n"
            "Convert them once with:\n"
            "  uv run --isolated --no-project --python 3.12 "
            "--with 'demucs-mlx[convert]==1.4.14' python scripts/convert_weights.py"
        ) from exc


def load_audio(path: str | Path, sr: int, start: float = 0.0, end: float | None = None) -> np.ndarray:
    """Decode ``path`` at ``sr`` and return float32 stereo audio shaped (channels, samples)."""
    import mlx_audio_io as mac

    duration = None if end is None else end - start
    audio, _ = mac.load(str(path), sr=sr, offset=start, duration=duration, dtype="float32")
    audio = np.asarray(audio).T  # channels_last → (channels, samples)
    if audio.shape[0] == 1:
        audio = np.repeat(audio, 2, axis=0)
    if audio.shape[1] == 0:
        raise ValueError(f"no audio decoded from {path} (check the time range)")
    return audio


def separate(separator, audio: np.ndarray) -> dict[str, np.ndarray]:
    """Run the model; returns ``{stem_name: (channels, samples)}``."""
    _, stems = separator.separate_tensor(audio)
    return stems


def split_guitar(stems: dict[str, np.ndarray]) -> tuple[np.ndarray, np.ndarray]:
    """Return ``(guitar, backing)`` where backing is the unity-gain sum of every other stem."""
    if GUITAR not in stems:
        raise ValueError(f"model has no guitar stem (stems: {', '.join(stems)})")
    others = [audio for name, audio in stems.items() if name != GUITAR]
    backing = np.sum(others, axis=0, dtype=np.float32)
    return stems[GUITAR], backing


def write_wav(path: str | Path, audio: np.ndarray, sr: int) -> Path:
    """Write (channels, samples) audio as 32-bit float WAV, so summed stems never clip."""
    path = Path(path)
    sf.write(path, audio.T, sr, subtype="FLOAT")
    return path
