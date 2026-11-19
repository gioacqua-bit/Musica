//! Fallback decoder: any format Symphonia understands → 16-bit PCM WAV.
//!
//! The webview plays most files directly. Codec support differs per platform (for example,
//! macOS's WebKit has no Ogg Vorbis, and Linux's WebKitGTK depends on the installed GStreamer
//! plugins), so when the webview refuses a file the interface asks for this WAV instead, which
//! every webview can play and analyse.

use std::fs::File;
use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub fn to_wav(path: &Path) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| format!("unsupported format: {e}"))?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or("no audio track")?;
    let track_id = track.id;
    let mut rate = track.codec_params.sample_rate.unwrap_or(44_100);
    let mut channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(2);
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("unsupported codec: {e}"))?;

    let mut pcm: Vec<i16> = Vec::new();
    let mut buf: Option<(SampleBuffer<i16>, usize)> = None; // (buffer, frame capacity)
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(Error::ResetRequired) => break,
            Err(e) => return Err(e.to_string()),
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                rate = spec.rate;
                channels = spec.channels.count();
                let frames = decoded.capacity();
                if buf.as_ref().map_or(true, |(_, cap)| *cap < frames) {
                    buf = Some((SampleBuffer::new(frames as u64, spec), frames));
                }
                let (sb, _) = buf.as_mut().unwrap();
                sb.copy_interleaved_ref(decoded);
                pcm.extend_from_slice(sb.samples());
            }
            Err(Error::DecodeError(_)) => continue, // skip a corrupt packet, keep going
            Err(e) => return Err(e.to_string()),
        }
    }
    if pcm.is_empty() {
        return Err("no audio decoded".into());
    }
    Ok(wav(&pcm, rate, channels as u16))
}

fn wav(samples: &[i16], rate: u32, channels: u16) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&(channels * 2).to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_every_file_from_env() {
        let Ok(dir) = std::env::var("MUSICA_TEST_DIR") else { return };
        for e in walkdir::WalkDir::new(dir).into_iter().filter_map(Result::ok).filter(|e| e.file_type().is_file()) {
            let p = e.path();
            let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
            if !["mp3", "flac", "wav", "ogg", "opus", "m4a", "aiff"].contains(&ext.as_str()) || e.file_name().to_string_lossy().starts_with('.') { continue; }
            match to_wav(p) {
                Ok(w) => {
                    let rate = u32::from_le_bytes(w[24..28].try_into().unwrap());
                    let ch = u16::from_le_bytes(w[22..24].try_into().unwrap());
                    let secs = (w.len() - 44) as f64 / (rate as f64 * ch as f64 * 2.0);
                    println!("OK   {:<40} {} Hz, {} ch, {:.2} s", e.file_name().to_string_lossy(), rate, ch, secs);
                }
                Err(err) => println!("FAIL {:<40} {}", e.file_name().to_string_lossy(), err),
            }
        }
    }
}
