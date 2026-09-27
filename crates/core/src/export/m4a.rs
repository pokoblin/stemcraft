//! AAC-LC 256 kbps CBR in an MPEG-4 (.m4a) container via macOS AudioToolbox.

use std::ffi::c_void;
use std::path::Path;
use std::ptr::{self, NonNull};

use anyhow::{anyhow, bail, Context, Result};
use objc2_audio_toolbox::{
    kAudioCodecBitRateControlMode_Constant, kAudioCodecPropertyBitRateControlMode,
    kAudioConverterEncodeBitRate, kAudioFileM4AType, kExtAudioFileProperty_AudioConverter,
    kExtAudioFileProperty_ClientDataFormat, kExtAudioFileProperty_ConverterConfig,
    AudioConverterRef, AudioConverterSetProperty, AudioFileFlags, ExtAudioFileCreateWithURL,
    ExtAudioFileDispose, ExtAudioFileGetProperty, ExtAudioFileRef, ExtAudioFileSetProperty,
    ExtAudioFileWrite,
};
use objc2_core_audio_types::{
    kAudioFormatFlagIsFloat, kAudioFormatFlagIsPacked, kAudioFormatLinearPCM,
    kAudioFormatMPEG4AAC, AudioBuffer, AudioBufferList, AudioStreamBasicDescription,
};
use objc2_core_foundation::CFURL;

use crate::audio::StereoAudio;

const CHUNK_FRAMES: usize = 4096;
const BITRATE: u32 = 256_000;

fn check(status: i32, what: &str) -> Result<()> {
    if status == 0 {
        return Ok(());
    }
    // Many CoreAudio errors are four-character codes.
    let bytes = status.to_be_bytes();
    let code = if bytes.iter().all(|c| c.is_ascii_graphic()) {
        format!(" '{}'", String::from_utf8_lossy(&bytes))
    } else {
        String::new()
    };
    bail!("{what} failed: OSStatus {status}{code}")
}

/// Disposes the ExtAudioFileRef on drop, so an early error doesn't leak it.
struct ExtFile(ExtAudioFileRef);

impl ExtFile {
    /// Dispose flushes the encoder and finalizes the file, so its status matters.
    fn close(mut self) -> Result<()> {
        let file = std::mem::replace(&mut self.0, ptr::null_mut());
        check(unsafe { ExtAudioFileDispose(file) }, "ExtAudioFileDispose")
    }
}

impl Drop for ExtFile {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { ExtAudioFileDispose(self.0) };
        }
    }
}

/// Expects samples in ±1.0 at a rate AAC accepts (see `super::lossy_rate`).
pub(super) fn write(path: &Path, audio: &StereoAudio) -> Result<()> {
    let mut file_format = AudioStreamBasicDescription {
        mSampleRate: audio.sample_rate as f64,
        mFormatID: kAudioFormatMPEG4AAC,
        mFormatFlags: 0,
        mBytesPerPacket: 0,
        mFramesPerPacket: 1024,
        mBytesPerFrame: 0,
        mChannelsPerFrame: 2,
        mBitsPerChannel: 0,
        mReserved: 0,
    };
    let client_format = AudioStreamBasicDescription {
        mSampleRate: audio.sample_rate as f64,
        mFormatID: kAudioFormatLinearPCM,
        mFormatFlags: kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked,
        mBytesPerPacket: 8,
        mFramesPerPacket: 1,
        mBytesPerFrame: 8,
        mChannelsPerFrame: 2,
        mBitsPerChannel: 32,
        mReserved: 0,
    };
    let url = CFURL::from_file_path(path)
        .ok_or_else(|| anyhow!("cannot build a URL for {}", path.display()))?;

    let mut raw: ExtAudioFileRef = ptr::null_mut();
    check(
        unsafe {
            ExtAudioFileCreateWithURL(
                &url,
                kAudioFileM4AType,
                NonNull::from(&mut file_format),
                ptr::null(),
                AudioFileFlags::EraseFile.0,
                NonNull::from(&mut raw),
            )
        },
        "ExtAudioFileCreateWithURL",
    )
    .with_context(|| format!("cannot create {}", path.display()))?;
    let file = ExtFile(raw);

    unsafe {
        check(
            ExtAudioFileSetProperty(
                file.0,
                kExtAudioFileProperty_ClientDataFormat,
                size_of::<AudioStreamBasicDescription>() as u32,
                NonNull::from(&client_format).cast::<c_void>(),
            ),
            "set client data format",
        )?;

        let mut converter: AudioConverterRef = ptr::null_mut();
        let mut size = size_of::<AudioConverterRef>() as u32;
        check(
            ExtAudioFileGetProperty(
                file.0,
                kExtAudioFileProperty_AudioConverter,
                NonNull::from(&mut size),
                NonNull::from(&mut converter).cast::<c_void>(),
            ),
            "get audio converter",
        )?;
        if converter.is_null() {
            bail!("AudioToolbox returned no audio converter");
        }
        // The default mode is average-bitrate, which lands far below 256 kbps on
        // simple material. Constant mode must be set before the bitrate.
        let mode: u32 = kAudioCodecBitRateControlMode_Constant;
        check(
            AudioConverterSetProperty(
                converter,
                kAudioCodecPropertyBitRateControlMode,
                size_of::<u32>() as u32,
                NonNull::from(&mode).cast::<c_void>(),
            ),
            "set bitrate mode",
        )?;
        let bitrate: u32 = BITRATE;
        check(
            AudioConverterSetProperty(
                converter,
                kAudioConverterEncodeBitRate,
                size_of::<u32>() as u32,
                NonNull::from(&bitrate).cast::<c_void>(),
            ),
            "set bitrate",
        )?;
        // A null config makes ExtAudioFile pick up the converter changes.
        let null_config: *const c_void = ptr::null();
        check(
            ExtAudioFileSetProperty(
                file.0,
                kExtAudioFileProperty_ConverterConfig,
                size_of::<*const c_void>() as u32,
                NonNull::from(&null_config).cast::<c_void>(),
            ),
            "apply converter settings",
        )?;
    }

    let mut interleaved = vec![0f32; CHUNK_FRAMES * 2];
    let mut pos = 0;
    while pos < audio.len() {
        let n = (audio.len() - pos).min(CHUNK_FRAMES);
        for i in 0..n {
            interleaved[2 * i] = audio.left[pos + i];
            interleaved[2 * i + 1] = audio.right[pos + i];
        }
        let mut buffers = AudioBufferList {
            mNumberBuffers: 1,
            mBuffers: [AudioBuffer {
                mNumberChannels: 2,
                mDataByteSize: (n * 2 * size_of::<f32>()) as u32,
                mData: interleaved.as_mut_ptr().cast::<c_void>(),
            }],
        };
        check(
            unsafe { ExtAudioFileWrite(file.0, n as u32, NonNull::from(&mut buffers)) },
            "ExtAudioFileWrite",
        )?;
        pos += n;
    }
    file.close()
}
