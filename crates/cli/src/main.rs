//! `stemcraft song.mp3` — isolated guitar + guitar-free backing track.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;
use stemcraft_core::audio::{self, StereoAudio};
use stemcraft_core::chords::{self, ChordOptions, NO_CHORD};
use stemcraft_core::separation::Separator;
use stemcraft_core::timerange::TimeRange;
use stemcraft_core::weights;
use indicatif::{ProgressBar, ProgressStyle};

/// Split a song into an isolated guitar track and a guitar-free backing track
/// (Demucs htdemucs_6s on the GPU).
#[derive(Parser)]
#[command(name = "stemcraft", version)]
struct Cli {
    /// Audio file (wav, aiff, flac, mp3, ogg, m4a)
    input: PathBuf,

    /// Output root; each song gets its own subfolder
    #[arg(short, long, default_value = "output")]
    out: PathBuf,

    /// Process only part of the song, e.g. 1:30-3:00, 90-, -2:00
    #[arg(short, long = "range", value_name = "START-END")]
    range: Option<TimeRange>,

    /// Detect chords from the guitar stem (.lrc and .txt)
    #[arg(short, long)]
    chords: bool,

    /// Also write every raw stem (drums, bass, other, vocals, guitar, piano)
    #[arg(long)]
    all_stems: bool,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

/// Keep Unicode titles (macOS filenames are Unicode); drop only path separators.
fn safe_name(stem: &str) -> String {
    let cleaned: String = stem
        .chars()
        .map(|c| {
            if c == '/' || c == ':' || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches(|c| c == '.' || c == ' ');
    if trimmed.is_empty() {
        "output".into()
    } else {
        trimmed.into()
    }
}

fn bar(len: u64, template: &str) -> ProgressBar {
    let pb = ProgressBar::new(len);
    pb.set_style(
        ProgressStyle::with_template(template)
            .unwrap()
            .progress_chars("#>-"),
    );
    pb
}

fn run(cli: Cli) -> Result<()> {
    let t0 = Instant::now();
    let song = safe_name(&cli.input.file_stem().unwrap_or_default().to_string_lossy());

    let mut input = audio::decode(&cli.input)
        .with_context(|| format!("could not read {}", cli.input.display()))?;
    if let Some(range) = cli.range {
        input.trim(range);
        anyhow::ensure!(!input.is_empty(), "the time range lies outside the song");
    }
    println!(
        "🎵 {}  ({:.1}s @ {} Hz)",
        cli.input.file_name().unwrap_or_default().to_string_lossy(),
        input.duration_secs(),
        input.sample_rate
    );

    let weights = if weights::is_cached() {
        weights::load(|_, _| {})?
    } else {
        println!(
            "⬇️  Downloading {} weights (first run only)…",
            weights::MODEL.id
        );
        let pb = bar(0, "   [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})");
        let data = weights::load(|received, total| {
            if let Some(total) = total {
                pb.set_length(total);
            }
            pb.set_position(received);
        })?;
        pb.finish_and_clear();
        data
    };
    let separator = Separator::new(&weights)?;
    if Separator::needs_warmup() {
        println!("⚙️  Compiling GPU kernels (first run only)…");
        separator.warmup();
    }

    println!("🎸 Separating with {} …", weights::MODEL.id);
    let t_sep = Instant::now();
    let pb = bar(1000, "   [{bar:40.cyan/blue}] {percent}% ({eta})");
    let stems = separator.separate(&input, |f| pb.set_position((f * 1000.0) as u64))?;
    pb.finish_and_clear();
    println!("   done in {:.1}s", t_sep.elapsed().as_secs_f64());

    let out_dir = cli.out.join(&song);
    std::fs::create_dir_all(&out_dir)
        .with_context(|| format!("cannot create {}", out_dir.display()))?;
    let mut written: Vec<PathBuf> = Vec::new();
    let mut write = |path: PathBuf, track: &StereoAudio| -> Result<()> {
        audio::write_wav(&path, track)?;
        written.push(path);
        Ok(())
    };

    let (guitar, backing) = stems.guitar_and_backing()?;
    write(out_dir.join(format!("{song}_guitar.wav")), &guitar)?;
    write(out_dir.join(format!("{song}_no_guitar.wav")), &backing)?;
    if cli.all_stems {
        let stem_dir = out_dir.join("stems");
        std::fs::create_dir_all(&stem_dir)?;
        for (name, track) in &stems.tracks {
            write(stem_dir.join(format!("{song}_{name}.wav")), track)?;
        }
    }

    if cli.chords {
        println!("🎼 Detecting chords …");
        let found = chords::detect_chords(
            &guitar.to_mono(),
            guitar.sample_rate,
            &ChordOptions::default(),
        );
        let lrc = out_dir.join(format!("{song}_chords.lrc"));
        let txt = out_dir.join(format!("{song}_chords.txt"));
        chords::write_lrc(&found, &lrc)?;
        chords::write_txt(&found, &txt, &song)?;
        written.extend([lrc, txt]);
        println!(
            "   {} chord segments",
            found.iter().filter(|c| c.label != NO_CHORD).count()
        );
    }

    println!(
        "✅ Finished in {:.1}s → {}",
        t0.elapsed().as_secs_f64(),
        out_dir.display()
    );
    for path in &written {
        println!(
            "   {}",
            path.strip_prefix(&out_dir)
                .unwrap_or(Path::new(path))
                .display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::safe_name;

    #[test]
    fn safe_name_keeps_unicode() {
        assert_eq!(safe_name("测试 歌曲"), "测试 歌曲");
        assert_eq!(safe_name("AC/DC: Live"), "AC_DC_ Live");
        assert_eq!(safe_name(" .. "), "output");
    }
}
