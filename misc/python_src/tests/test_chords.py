import numpy as np

from stemcraft.chords import NO_CHORD, Chord, detect_chords, write_lrc

SR = 22050


def triad(root_midi: int, minor: bool, seconds: float) -> np.ndarray:
    t = np.arange(int(seconds * SR)) / SR
    notes = (root_midi, root_midi + (3 if minor else 4), root_midi + 7)
    y = np.zeros_like(t)
    for n in notes:
        f = 440.0 * 2 ** ((n - 69) / 12)
        y += np.sin(2 * np.pi * f * t) + 0.3 * np.sin(4 * np.pi * f * t)
    return (0.2 * y).astype(np.float32)


def test_detects_progression_with_silence():
    # C major, A minor, silence, G major.
    y = np.concatenate([
        triad(60, False, 2.0),
        triad(57, True, 2.0),
        np.zeros(SR, dtype=np.float32),
        triad(55, False, 2.0),
    ])
    found = detect_chords(y, SR)
    assert [c.label for c in found] == ["C", "Am", NO_CHORD, "G"]
    for c, expected_start in zip(found, (0.0, 2.0, 4.0, 5.0)):
        assert abs(c.start - expected_start) < 0.3
    assert found[-1].end == len(y) / SR


def test_stereo_input_and_other_sample_rate():
    mono = triad(62, True, 3.0)  # D minor
    stereo = np.stack([mono, mono])
    import librosa
    stereo_44k = librosa.resample(stereo, orig_sr=SR, target_sr=44100)
    assert [c.label for c in detect_chords(stereo_44k, 44100)] == ["Dm"]


def test_silence_is_no_chord():
    assert [c.label for c in detect_chords(np.zeros(SR * 2, dtype=np.float32), SR)] == [NO_CHORD]


def test_write_lrc(tmp_path):
    path = write_lrc([Chord(0.0, 2.0, "C"), Chord(62.5, 64.0, "Am")], tmp_path / "c.lrc")
    assert path.read_text(encoding="utf-8") == "[00:00.00]C\n[01:02.50]Am\n"
