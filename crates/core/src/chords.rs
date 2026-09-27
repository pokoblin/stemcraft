//! Chord detection on an isolated guitar stem by chroma template matching.
//!
//! STFT chroma → fixed-length blocks → cosine match against the 24 major/minor
//! triads → energy gate ("N.C." over silence) → majority-vote smoothing →
//! run-length merge. Reports triad names only (no 7ths, sus, inversions).

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;

use anyhow::Result;
use realfft::RealFftPlanner;

pub const NO_CHORD: &str = "N.C.";
const PITCHES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];
/// Pitch range folded into the chroma: A1 (low guitar strings) to A6.
const MIN_FREQ: f32 = 55.0;
const MAX_FREQ: f32 = 1760.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Chord {
    pub start: f64,
    pub end: f64,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct ChordOptions {
    /// Analysis block length in seconds.
    pub block_secs: f64,
    /// Majority-vote window, in blocks.
    pub smooth_blocks: usize,
    /// Segments shorter than this are folded into their predecessor.
    pub min_duration: f64,
    /// Blocks quieter than this fraction of the peak RMS become "N.C.".
    pub energy_gate: f32,
}

impl Default for ChordOptions {
    fn default() -> Self {
        Self {
            block_secs: 0.25,
            smooth_blocks: 5,
            min_duration: 0.5,
            energy_gate: 0.08,
        }
    }
}

/// 24 L2-normalised triad templates. Root and third outweigh the fifth, which
/// relative major/minor pairs share.
fn templates() -> Vec<([f32; 12], String)> {
    let mut out = Vec::with_capacity(24);
    for (root, name) in PITCHES.iter().enumerate() {
        for (third, suffix) in [(4, ""), (3, "m")] {
            let mut t = [0.0f32; 12];
            t[root] = 1.2;
            t[(root + third) % 12] = 1.0;
            t[(root + 7) % 12] = 0.8;
            let norm = t.iter().map(|x| x * x).sum::<f32>().sqrt();
            t.iter_mut().for_each(|x| *x /= norm);
            out.push((t, format!("{name}{suffix}")));
        }
    }
    out
}

/// Per-frame chroma (max-normalised) and RMS of a centred STFT.
fn chroma_frames(y: &[f32], sr: u32, n_fft: usize, hop: usize) -> (Vec<[f32; 12]>, Vec<f32>) {
    // Each FFT bin in range contributes to its nearest pitch class, weighted by
    // how close it lies to the semitone centre.
    let bin_hz = sr as f32 / n_fft as f32;
    let bins: Vec<(usize, usize, f32)> = (1..n_fft / 2)
        .filter_map(|k| {
            let f = k as f32 * bin_hz;
            if !(MIN_FREQ..=MAX_FREQ).contains(&f) {
                return None;
            }
            let midi = 69.0 + 12.0 * (f / 440.0).log2();
            let nearest = midi.round();
            let weight = 1.0 - 2.0 * (midi - nearest).abs();
            (weight > 0.0).then_some((k, nearest.rem_euclid(12.0) as usize, weight))
        })
        .collect();

    let window: Vec<f32> = (0..n_fft)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / n_fft as f32).cos())
        .collect();
    let fft = RealFftPlanner::<f32>::new().plan_fft_forward(n_fft);
    let mut frame = fft.make_input_vec();
    let mut spectrum = fft.make_output_vec();

    let n_frames = y.len() / hop + 1;
    let half = n_fft / 2;
    let mut chroma = Vec::with_capacity(n_frames);
    let mut rms = Vec::with_capacity(n_frames);
    for i in 0..n_frames {
        let centre = i * hop;
        let mut energy = 0.0f32;
        for (j, slot) in frame.iter_mut().enumerate() {
            let s = (centre + j)
                .checked_sub(half)
                .and_then(|idx| y.get(idx))
                .copied()
                .unwrap_or(0.0);
            energy += s * s;
            *slot = s * window[j];
        }
        rms.push((energy / n_fft as f32).sqrt());
        fft.process(&mut frame, &mut spectrum)
            .expect("buffer sizes match the plan");

        let mut c = [0.0f32; 12];
        for &(k, pc, w) in &bins {
            c[pc] += w * spectrum[k].norm_sqr();
        }
        let max = c.iter().copied().fold(0.0, f32::max);
        if max > 0.0 {
            c.iter_mut().for_each(|x| *x /= max);
        }
        chroma.push(c);
    }
    (chroma, rms)
}

fn median(values: &mut [f32]) -> f32 {
    values.sort_by(f32::total_cmp);
    let n = values.len();
    if n % 2 == 1 {
        values[n / 2]
    } else {
        0.5 * (values[n / 2 - 1] + values[n / 2])
    }
}

/// Detect chords in mono audio `y` sampled at `sr`.
pub fn detect_chords(y: &[f32], sr: u32, opts: &ChordOptions) -> Vec<Chord> {
    if y.is_empty() {
        return Vec::new();
    }
    let duration = y.len() as f64 / sr as f64;
    // ~5 Hz bins at any common sample rate: 4096 @ 22.05 kHz, 8192 @ 44.1/48 kHz.
    let n_fft = ((sr as f64 * 0.186) as usize).next_power_of_two();
    let hop = n_fft / 8;
    let (chroma, rms) = chroma_frames(y, sr, n_fft, hop);
    let peak = rms.iter().copied().fold(0.0, f32::max);
    if peak <= 0.0 {
        return vec![Chord {
            start: 0.0,
            end: duration,
            label: NO_CHORD.into(),
        }];
    }

    let templates = templates();
    let block = ((opts.block_secs * sr as f64 / hop as f64).round() as usize).max(1);
    let labels: Vec<&str> = chroma
        .chunks(block)
        .zip(rms.chunks(block))
        .map(|(frames, energies)| {
            if median(&mut energies.to_vec()) < opts.energy_gate * peak {
                return NO_CHORD;
            }
            let mut vec = [0.0f32; 12];
            for (pc, v) in vec.iter_mut().enumerate() {
                *v = median(&mut frames.iter().map(|f| f[pc]).collect::<Vec<_>>());
            }
            let norm = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm < 1e-6 {
                return NO_CHORD;
            }
            let score = |t: &[f32; 12]| t.iter().zip(&vec).map(|(a, b)| a * b).sum::<f32>();
            let best = templates
                .iter()
                .max_by(|a, b| score(&a.0).total_cmp(&score(&b.0)))
                .expect("24 templates");
            best.1.as_str()
        })
        .collect();

    let smoothed = majority_vote(&labels, opts.smooth_blocks / 2);
    let block_time = |b: usize| ((b * block * hop) as f64 / sr as f64).min(duration);

    let mut segments: Vec<Chord> = Vec::new();
    let mut run_start = 0;
    for i in 1..=smoothed.len() {
        if i == smoothed.len() || smoothed[i] != smoothed[run_start] {
            let end = if i == smoothed.len() {
                duration
            } else {
                block_time(i)
            };
            segments.push(Chord {
                start: block_time(run_start),
                end,
                label: smoothed[run_start].into(),
            });
            run_start = i;
        }
    }
    absorb_short(segments, opts.min_duration)
}

/// Most common label in a `±half` window; ties go to the label seen first.
fn majority_vote<'a>(labels: &[&'a str], half: usize) -> Vec<&'a str> {
    (0..labels.len())
        .map(|i| {
            let window = &labels[i.saturating_sub(half)..(i + half + 1).min(labels.len())];
            let mut counts: HashMap<&str, usize> = HashMap::new();
            for l in window {
                *counts.entry(l).or_default() += 1;
            }
            let best = counts.values().copied().max().unwrap_or(0);
            *window
                .iter()
                .find(|l| counts[*l] == best)
                .expect("non-empty window")
        })
        .collect()
}

/// Fold segments shorter than `min_duration` into their predecessor, then re-merge runs.
fn absorb_short(segments: Vec<Chord>, min_duration: f64) -> Vec<Chord> {
    let mut out: Vec<Chord> = Vec::with_capacity(segments.len());
    for seg in segments {
        match out.last_mut() {
            Some(prev) if seg.end - seg.start < min_duration || seg.label == prev.label => {
                prev.end = seg.end
            }
            _ => out.push(seg),
        }
    }
    out
}

/// LRC-style timestamps so synced-lyric players can show the chart.
pub fn write_lrc(chords: &[Chord], path: &Path) -> Result<()> {
    let mut text = String::new();
    for c in chords {
        let minutes = (c.start / 60.0).floor();
        writeln!(
            text,
            "[{:02}:{:05.2}]{}",
            minutes as u64,
            c.start - minutes * 60.0,
            c.label
        )?;
    }
    Ok(std::fs::write(path, text)?)
}

pub fn write_txt(chords: &[Chord], path: &Path, title: &str) -> Result<()> {
    let mut text = format!("# Chords — {title}\n\n");
    for c in chords {
        let secs = c.start as u64;
        writeln!(
            text,
            "{:>7}   {}",
            format!("{}:{:02}", secs / 60, secs % 60),
            c.label
        )?;
    }
    Ok(std::fs::write(path, text)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 22050;

    fn triad(root_midi: i32, minor: bool, secs: f64, sr: u32) -> Vec<f32> {
        let notes = [
            root_midi,
            root_midi + if minor { 3 } else { 4 },
            root_midi + 7,
        ];
        (0..(secs * sr as f64) as usize)
            .map(|i| {
                let t = i as f32 / sr as f32;
                0.2 * notes
                    .iter()
                    .map(|&n| {
                        let f = 440.0 * 2f32.powf((n - 69) as f32 / 12.0);
                        let w = 2.0 * std::f32::consts::PI * f * t;
                        w.sin() + 0.3 * (2.0 * w).sin()
                    })
                    .sum::<f32>()
            })
            .collect()
    }

    fn labels(chords: &[Chord]) -> Vec<&str> {
        chords.iter().map(|c| c.label.as_str()).collect()
    }

    #[test]
    fn detects_progression_with_silence() {
        // C major, A minor, silence, G major.
        let mut y = triad(60, false, 2.0, SR);
        y.extend(triad(57, true, 2.0, SR));
        y.extend(vec![0.0; SR as usize]);
        y.extend(triad(55, false, 2.0, SR));
        let found = detect_chords(&y, SR, &ChordOptions::default());
        assert_eq!(labels(&found), ["C", "Am", NO_CHORD, "G"]);
        for (c, expected) in found.iter().zip([0.0, 2.0, 4.0, 5.0]) {
            assert!(
                (c.start - expected).abs() < 0.3,
                "{c:?} should start near {expected}"
            );
        }
        assert_eq!(found.last().unwrap().end, y.len() as f64 / SR as f64);
    }

    #[test]
    fn works_at_44k() {
        let y = triad(62, true, 3.0, 44_100); // D minor
        assert_eq!(
            labels(&detect_chords(&y, 44_100, &ChordOptions::default())),
            ["Dm"]
        );
    }

    #[test]
    fn silence_is_no_chord() {
        let found = detect_chords(&vec![0.0; 2 * SR as usize], SR, &ChordOptions::default());
        assert_eq!(labels(&found), [NO_CHORD]);
    }

    #[test]
    fn writes_lrc_and_txt() {
        let dir = tempfile::tempdir().unwrap();
        let chords = [
            Chord {
                start: 0.0,
                end: 2.0,
                label: "C".into(),
            },
            Chord {
                start: 62.5,
                end: 64.0,
                label: "Am".into(),
            },
        ];
        write_lrc(&chords, &dir.path().join("c.lrc")).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("c.lrc")).unwrap(),
            "[00:00.00]C\n[01:02.50]Am\n"
        );
        write_txt(&chords, &dir.path().join("c.txt"), "song").unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("c.txt")).unwrap(),
            "# Chords — song\n\n   0:00   C\n   1:02   Am\n"
        );
    }
}
