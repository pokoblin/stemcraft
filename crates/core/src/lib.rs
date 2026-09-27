//! Stemcraft core: decode audio, split it with HTDemucs (6 stems) on the
//! GPU, build the guitar / guitar-free backing tracks, and detect chords.
//! Shared by the CLI and (later) the GPUI desktop app.

pub mod audio;
pub mod chords;
pub mod separation;
pub mod timerange;
pub mod weights;
