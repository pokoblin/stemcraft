//! Live mute / solo / volume per track: the UI writes, the audio callback reads.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};

use stemcraft_core::mix::TrackMix;

struct TrackControl {
    mute: AtomicBool,
    solo: AtomicBool,
    /// f32 bits.
    volume: AtomicU32,
}

pub struct MixControls {
    tracks: Vec<TrackControl>,
}

impl MixControls {
    pub fn new(tracks: usize) -> Self {
        Self {
            tracks: (0..tracks)
                .map(|_| TrackControl {
                    mute: AtomicBool::new(false),
                    solo: AtomicBool::new(false),
                    volume: AtomicU32::new(1.0f32.to_bits()),
                })
                .collect(),
        }
    }

    pub fn get(&self, i: usize) -> TrackMix {
        let t = &self.tracks[i];
        TrackMix {
            mute: t.mute.load(Relaxed),
            solo: t.solo.load(Relaxed),
            volume: f32::from_bits(t.volume.load(Relaxed)),
        }
    }

    pub fn set_mute(&self, i: usize, on: bool) {
        self.tracks[i].mute.store(on, Relaxed);
    }

    pub fn set_solo(&self, i: usize, on: bool) {
        self.tracks[i].solo.store(on, Relaxed);
    }

    pub fn set_volume(&self, i: usize, volume: f32) {
        self.tracks[i].volume.store(volume.clamp(0.0, 1.0).to_bits(), Relaxed);
    }

    /// Allocation-free when `out` already has the capacity (audio thread).
    pub fn read_into(&self, out: &mut Vec<TrackMix>) {
        out.clear();
        out.extend((0..self.tracks.len()).map(|i| self.get(i)));
    }

    pub fn snapshot(&self) -> Vec<TrackMix> {
        let mut out = Vec::with_capacity(self.tracks.len());
        self.read_into(&mut out);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_unmuted_at_full_volume() {
        let c = MixControls::new(2);
        assert_eq!(c.snapshot(), vec![TrackMix::default(); 2]);
    }

    #[test]
    fn setters_update_one_track() {
        let c = MixControls::new(3);
        c.set_mute(0, true);
        c.set_solo(1, true);
        c.set_volume(2, 0.25);
        c.set_volume(1, 7.0);
        assert_eq!(c.get(0), TrackMix { mute: true, solo: false, volume: 1.0 });
        assert_eq!(c.get(1), TrackMix { mute: false, solo: true, volume: 1.0 });
        assert_eq!(c.get(2).volume, 0.25);
    }

    #[test]
    fn read_into_replaces_contents() {
        let c = MixControls::new(2);
        let mut out = vec![TrackMix { mute: true, solo: true, volume: 0.0 }; 5];
        c.read_into(&mut out);
        assert_eq!(out, vec![TrackMix::default(); 2]);
    }
}
