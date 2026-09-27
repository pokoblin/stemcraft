import numpy as np
import pytest
import soundfile as sf

from stemcraft.separation import split_guitar, write_wav


def test_split_guitar_sums_every_other_stem():
    rng = np.random.default_rng(0)
    stems = {n: rng.standard_normal((2, 100)).astype(np.float32)
             for n in ("drums", "bass", "vocals", "piano", "other", "guitar")}
    guitar, backing = split_guitar(stems)
    assert guitar is stems["guitar"]
    expected = sum(stems[n] for n in ("drums", "bass", "vocals", "piano", "other"))
    np.testing.assert_allclose(backing, expected, rtol=1e-6)
    np.testing.assert_allclose(guitar + backing, sum(stems.values()), rtol=1e-5, atol=1e-6)


def test_split_guitar_requires_guitar_stem():
    with pytest.raises(ValueError, match="no guitar stem"):
        split_guitar({"drums": np.zeros((2, 4)), "bass": np.zeros((2, 4))})


def test_write_wav_keeps_values_above_full_scale(tmp_path):
    audio = np.array([[0.5, 1.7, -2.0], [0.0, -1.5, 1.2]], dtype=np.float32)
    path = write_wav(tmp_path / "x.wav", audio, 44100)
    data, sr = sf.read(path, dtype="float32")
    assert sr == 44100
    np.testing.assert_array_equal(data.T, audio)
