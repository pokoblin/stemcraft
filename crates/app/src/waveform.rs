//! Waveform overview data: min/max peaks, precomputed once per track.

use stemcraft_core::audio::StereoAudio;

/// Enough detail for a full-width window; columns() rebuckets to pixels.
pub const PEAK_BUCKETS: usize = 4000;

/// Min/max of the mono downmix in `buckets` equal slices.
pub fn peaks(audio: &StereoAudio, buckets: usize) -> Vec<(f32, f32)> {
    let len = audio.len();
    if len == 0 || buckets == 0 {
        return Vec::new();
    }
    let buckets = buckets.min(len);
    (0..buckets)
        .map(|b| {
            let (start, end) = (b * len / buckets, (b + 1) * len / buckets);
            (start..end).fold((f32::MAX, f32::MIN), |(lo, hi), i| {
                let m = 0.5 * (audio.left[i] + audio.right[i]);
                (lo.min(m), hi.max(m))
            })
        })
        .collect()
}

/// Rebucket peaks to exactly `n` drawing columns (merging or repeating).
pub fn columns(peaks: &[(f32, f32)], n: usize) -> Vec<(f32, f32)> {
    if peaks.is_empty() || n == 0 {
        return Vec::new();
    }
    (0..n)
        .map(|c| {
            let start = c * peaks.len() / n;
            let end = ((c + 1) * peaks.len() / n).max(start + 1);
            peaks[start..end]
                .iter()
                .fold((f32::MAX, f32::MIN), |(lo, hi), &(l, h)| (lo.min(l), hi.max(h)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audio(left: Vec<f32>, right: Vec<f32>) -> StereoAudio {
        StereoAudio { left, right, sample_rate: 10 }
    }

    #[test]
    fn peaks_are_min_max_of_the_mono_mix() {
        let a = audio(vec![1.0, -1.0, 0.5, 0.5], vec![1.0, 0.0, -0.5, 0.5]);
        // mono: [1.0, -0.5, 0.0, 0.5]
        assert_eq!(peaks(&a, 2), vec![(-0.5, 1.0), (0.0, 0.5)]);
    }

    #[test]
    fn peaks_never_exceed_the_sample_count() {
        let a = audio(vec![0.1, 0.2], vec![0.1, 0.2]);
        assert_eq!(peaks(&a, 100).len(), 2);
        assert!(peaks(&audio(vec![], vec![]), 10).is_empty());
    }

    #[test]
    fn columns_merge_or_repeat_peaks() {
        let p = [(-1.0, 0.1), (-0.2, 0.9), (-0.5, 0.5), (0.0, 0.2)];
        assert_eq!(columns(&p, 2), vec![(-1.0, 0.9), (-0.5, 0.5)]);
        assert_eq!(columns(&p[..1], 3), vec![(-1.0, 0.1); 3]);
        assert!(columns(&p, 0).is_empty());
    }
}
