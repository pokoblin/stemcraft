import pytest

from stemcraft.timerange import parse_clock, parse_range


@pytest.mark.parametrize(
    ("text", "seconds"),
    [("45", 45.0), ("1:30", 90.0), ("1:02:03", 3723.0), ("2:05.5", 125.5)],
)
def test_parse_clock(text, seconds):
    assert parse_clock(text) == seconds


@pytest.mark.parametrize(
    ("text", "expected"),
    [
        ("1:30-3:00", (90.0, 180.0)),
        ("90 - 180", (90.0, 180.0)),
        ("1:30-", (90.0, None)),
        ("-2:00", (0.0, 120.0)),
    ],
)
def test_parse_range(text, expected):
    assert parse_range(text) == expected


@pytest.mark.parametrize("text", ["3:00-1:30", "abc", "1:30", "1::2-3", ""])
def test_parse_range_rejects(text):
    with pytest.raises(ValueError):
        parse_range(text)
