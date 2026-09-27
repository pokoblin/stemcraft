"""
One-time conversion of the official Demucs htdemucs_6s checkpoint into the
demucs-mlx safe cache (~/.cache/demucs-mlx). Needs torch + demucs, so run it in
a throwaway environment rather than the app's own:

    uv run --isolated --python 3.12 --with 'demucs-mlx[convert]==1.4.14' \
        python scripts/convert_weights.py

Why the patch: demucs-mlx 1.4.14 validates checkpoint metadata and requires
every mapping key to be a string. htdemucs_6s ships integer keys in
``training_args.dset.test_mapping`` (training bookkeeping, unused at
inference), so the stock converter refuses it. We accept integer keys and keep
every other check intact.
"""
from __future__ import annotations

import sys
from pathlib import Path

from demucs_mlx import mlx_convert, secure_demucs

_original_validate = secure_demucs._validate_metadata_value


def _validate_allowing_int_keys(value, path, **kwargs):
    if isinstance(value, dict) and any(isinstance(k, int) and not isinstance(k, bool) for k in value):
        value = {str(k) if isinstance(k, int) else k: v for k, v in value.items()}
    return _original_validate(value, path, **kwargs)


secure_demucs._validate_metadata_value = _validate_allowing_int_keys


def main() -> None:
    out_dir = Path.home() / ".cache" / "demucs-mlx"
    sys.argv = [sys.argv[0], "htdemucs_6s", "--output-dir", str(out_dir), "--verify"]
    mlx_convert.main()


if __name__ == "__main__":
    main()
