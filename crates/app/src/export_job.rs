//! Export on a background thread: the mix, optional single stems, optional chord chart.

use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;

use anyhow::{Context, Result};
use stemcraft_core::audio::StereoAudio;
use stemcraft_core::chords::{write_lrc, write_txt, Chord};
use stemcraft_core::export::{self, ExportFormat};
use stemcraft_core::mix::mixdown;

use crate::naming::ExportPlan;

pub struct ExportJob {
    pub plan: ExportPlan,
    pub format: ExportFormat,
    pub stems: Arc<Vec<StereoAudio>>,
    /// Per-stem gains for the mix (from the mixer's controls).
    pub gains: Vec<f32>,
    /// Stems written individually, matching `plan.stems`.
    pub stem_indices: Vec<usize>,
    pub chords: Arc<Vec<Chord>>,
    /// Song title for the chord chart header.
    pub title: String,
}

impl ExportJob {
    pub fn total(&self) -> usize {
        1 + self.stem_indices.len() + usize::from(self.plan.chords.is_some())
    }
}

pub enum ExportMsg {
    Progress { done: usize, total: usize },
    Done(PathBuf),
    Failed(String),
}

/// `progress(done)` after each finished file.
pub fn run(job: &ExportJob, mut progress: impl FnMut(usize)) -> Result<()> {
    std::fs::create_dir_all(&job.plan.dir)
        .with_context(|| format!("cannot create {}", job.plan.dir.display()))?;
    let tracks: Vec<&StereoAudio> = job.stems.iter().collect();
    let mix = mixdown(&tracks, &job.gains)?;
    export::write(&job.plan.mix, &mix, job.format)?;
    drop(mix);
    let mut done = 1;
    progress(done);
    for (&i, path) in job.stem_indices.iter().zip(&job.plan.stems) {
        export::write(path, &job.stems[i], job.format)?;
        done += 1;
        progress(done);
    }
    if let Some((lrc, txt)) = &job.plan.chords {
        write_lrc(&job.chords, lrc)?;
        write_txt(&job.chords, txt, &job.title)?;
        done += 1;
        progress(done);
    }
    Ok(())
}

pub fn spawn(job: ExportJob) -> mpsc::Receiver<ExportMsg> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let total = job.total();
        let result = run(&job, |done| {
            let _ = tx.send(ExportMsg::Progress { done, total });
        });
        let _ = tx.send(match result {
            Ok(()) => ExportMsg::Done(job.plan.dir.clone()),
            Err(e) => ExportMsg::Failed(format!("{e:#}")),
        });
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::plan_export;
    use stemcraft_core::audio::decode;

    #[test]
    fn writes_mix_stems_and_chords() {
        let dir = tempfile::tempdir().unwrap();
        let a = StereoAudio { left: vec![0.1, 0.2], right: vec![0.3, 0.4], sample_rate: 8000 };
        let b = StereoAudio { left: vec![0.5, 0.5], right: vec![0.5, 0.5], sample_rate: 8000 };
        let job = ExportJob {
            plan: plan_export(dir.path(), "song", "wav", &["drums"], true),
            format: ExportFormat::Wav,
            stems: Arc::new(vec![a.clone(), b]),
            gains: vec![1.0, 0.5],
            stem_indices: vec![0],
            chords: Arc::new(vec![Chord { start: 0.0, end: 1.0, label: "C".into() }]),
            title: "song".into(),
        };
        assert_eq!(job.total(), 3);
        let mut seen = Vec::new();
        run(&job, |done| seen.push(done)).unwrap();
        assert_eq!(seen, vec![1, 2, 3]);

        let mix = decode(&job.plan.mix).unwrap();
        for (got, want) in mix.left.iter().zip([0.35, 0.45]) {
            assert!((got - want).abs() < 1e-6, "{got} vs {want}");
        }
        assert_eq!(decode(&job.plan.stems[0]).unwrap(), a);
        let (lrc, txt) = job.plan.chords.as_ref().unwrap();
        assert!(lrc.is_file() && txt.is_file());
    }

    #[test]
    fn spawn_reports_progress_then_done() {
        let dir = tempfile::tempdir().unwrap();
        let a = StereoAudio { left: vec![0.1], right: vec![0.1], sample_rate: 8000 };
        let job = ExportJob {
            plan: plan_export(dir.path(), "s", "wav", &[], false),
            format: ExportFormat::Wav,
            stems: Arc::new(vec![a]),
            gains: vec![1.0],
            stem_indices: vec![],
            chords: Arc::new(vec![]),
            title: "s".into(),
        };
        let expected_dir = job.plan.dir.clone();
        let msgs: Vec<ExportMsg> = spawn(job).iter().collect();
        assert!(matches!(msgs[0], ExportMsg::Progress { done: 1, total: 1 }));
        assert!(matches!(&msgs[1], ExportMsg::Done(d) if *d == expected_dir));
    }
}
