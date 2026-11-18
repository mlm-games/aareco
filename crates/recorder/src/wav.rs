use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::error::{RecorderError, Result, file_error};

const HEADER_LEN: u64 = 44;
const FORMAT_PCM: u16 = 1;
const BITS_PER_SAMPLE: u16 = 16;
const MAX_DATA_BYTES: u64 = u32::MAX as u64;

#[derive(Clone, Debug)]
pub struct WavInfo {
    pub path: PathBuf,
    pub sample_rate: u32,
    pub channels: u16,
    pub frames: u64,
}

impl WavInfo {
    pub fn duration_ms(&self) -> u64 {
        if self.sample_rate == 0 || self.channels == 0 {
            return 0;
        }
        self.frames.saturating_mul(1000) / (self.sample_rate as u64 * self.channels as u64)
    }
}

struct WavSpec {
    sample_rate: u32,
    channels: u16,
}

/// Writes 16-bit PCM WAV files in place. The header is patched on
/// [`WavWriter::finalize`]; files keep a `.part` suffix until then so a crash
/// leaves something [`recover_partial`] can finish.
pub struct WavWriter {
    file: BufWriter<File>,
    part_path: PathBuf,
    spec: WavSpec,
    data_bytes: u64,
    finalized: bool,
}

impl WavWriter {
    pub fn create(part_path: impl Into<PathBuf>, sample_rate: u32, channels: u16) -> Result<Self> {
        let part_path = part_path.into();
        if sample_rate == 0 || channels == 0 {
            return Err(RecorderError::UnsupportedFormat(format!(
                "{channels} channels at {sample_rate} Hz"
            )));
        }
        if let Some(parent) = part_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| file_error(parent, e))?;
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&part_path)
            .map_err(|e| file_error(&part_path, e))?;
        let mut file = BufWriter::new(file);
        write_header(&mut file, 0, sample_rate, channels)
            .map_err(|e| file_error(&part_path, e))?;
        file.flush().map_err(|e| file_error(&part_path, e))?;
        Ok(Self {
            file,
            part_path,
            spec: WavSpec {
                sample_rate,
                channels,
            },
            data_bytes: 0,
            finalized: false,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.spec.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.spec.channels
    }

    pub fn data_bytes(&self) -> u64 {
        self.data_bytes
    }

    pub fn write_samples(&mut self, samples: &[i16]) -> Result<()> {
        if samples.is_empty() {
            return Ok(());
        }
        let bytes = samples.len() as u64 * 2;
        if self.data_bytes.saturating_add(bytes) > MAX_DATA_BYTES {
            return Err(RecorderError::Start(
                "recording exceeded the 4 GB WAV limit".into(),
            ));
        }
        let mut buf = Vec::with_capacity(samples.len() * 2);
        for sample in samples {
            buf.extend_from_slice(&sample.to_le_bytes());
        }
        self.file
            .write_all(&buf)
            .map_err(|e| file_error(&self.part_path, e))?;
        self.data_bytes += bytes;
        Ok(())
    }

    /// Flush, patch the RIFF/data sizes, and rename `.part` to `.wav`.
    pub fn finalize(mut self) -> Result<WavInfo> {
        let info = self.finish()?;
        self.finalized = true;
        let wav_path = self.part_path.with_extension("wav");
        std::fs::rename(&self.part_path, &wav_path).map_err(|e| file_error(&self.part_path, e))?;
        Ok(WavInfo {
            path: wav_path,
            ..info
        })
    }

    /// Leave the file as a `.part` with a patched header. Used by tests and
    /// by callers that want recovery to run separately.
    pub fn finish_partial(mut self) -> Result<WavInfo> {
        let info = self.finish()?;
        self.finalized = true;
        Ok(info)
    }

    fn finish(&mut self) -> Result<WavInfo> {
        self.file
            .flush()
            .map_err(|e| file_error(&self.part_path, e))?;
        let data_len = self.data_bytes.min(MAX_DATA_BYTES) as u32;
        let riff_len = 36u32.saturating_add(data_len);
        let file = self.file.get_mut();
        file.seek(SeekFrom::Start(4))
            .and_then(|_| file.write_all(&riff_len.to_le_bytes()))
            .and_then(|_| file.seek(SeekFrom::Start(40)))
            .and_then(|_| file.write_all(&data_len.to_le_bytes()))
            .and_then(|_| file.flush())
            .and_then(|_| file.sync_all())
            .map_err(|e| file_error(&self.part_path, e))?;
        Ok(WavInfo {
            path: self.part_path.clone(),
            sample_rate: self.spec.sample_rate,
            channels: self.spec.channels,
            frames: self.data_bytes / 2,
        })
    }
}

impl Drop for WavWriter {
    fn drop(&mut self) {
        if !self.finalized {
            let _ = self.file.flush();
        }
    }
}

fn write_header(
    writer: &mut impl Write,
    data_len: u32,
    sample_rate: u32,
    channels: u16,
) -> std::io::Result<()> {
    let block_align = channels.saturating_mul(BITS_PER_SAMPLE / 8);
    let byte_rate = sample_rate.saturating_mul(block_align as u32);
    let riff_len = 36u32.saturating_add(data_len);

    writer.write_all(b"RIFF")?;
    writer.write_all(&riff_len.to_le_bytes())?;
    writer.write_all(b"WAVE")?;
    writer.write_all(b"fmt ")?;
    writer.write_all(&16u32.to_le_bytes())?;
    writer.write_all(&FORMAT_PCM.to_le_bytes())?;
    writer.write_all(&channels.to_le_bytes())?;
    writer.write_all(&sample_rate.to_le_bytes())?;
    writer.write_all(&byte_rate.to_le_bytes())?;
    writer.write_all(&block_align.to_le_bytes())?;
    writer.write_all(&BITS_PER_SAMPLE.to_le_bytes())?;
    writer.write_all(b"data")?;
    writer.write_all(&data_len.to_le_bytes())?;
    Ok(())
}

/// Finish a `.part` file left behind by a crash or a killed process.
/// Returns `Ok(None)` when the path is not a partial recording.
pub fn recover_partial(path: &Path) -> Result<Option<WavInfo>> {
    if path.extension().and_then(|e| e.to_str()) != Some("part") {
        return Ok(None);
    }
    let file_len = std::fs::metadata(path)
        .map_err(|e| file_error(path, e))?
        .len();
    if file_len < HEADER_LEN {
        return Err(file_error(
            path,
            std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "partial recording is shorter than a WAV header",
            ),
        ));
    }

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| file_error(path, e))?;
    let mut header = [0u8; HEADER_LEN as usize];
    file.read_exact(&mut header)
        .map_err(|e| file_error(path, e))?;
    let spec = parse_header(&header).map_err(|e| file_error(path, e))?;

    let data_bytes = (file_len - HEADER_LEN).min(MAX_DATA_BYTES);
    let riff_len = 36u32.saturating_add(data_bytes as u32);
    file.seek(SeekFrom::Start(4))
        .and_then(|_| file.write_all(&riff_len.to_le_bytes()))
        .and_then(|_| file.seek(SeekFrom::Start(40)))
        .and_then(|_| file.write_all(&(data_bytes as u32).to_le_bytes()))
        .and_then(|_| file.sync_all())
        .map_err(|e| file_error(path, e))?;
    drop(file);

    let wav_path = path.with_extension("wav");
    std::fs::rename(path, &wav_path).map_err(|e| file_error(path, e))?;
    Ok(Some(WavInfo {
        path: wav_path,
        sample_rate: spec.sample_rate,
        channels: spec.channels,
        frames: data_bytes / 2,
    }))
}

pub fn inspect_wav(path: &Path) -> Result<WavInfo> {
    let mut file = File::open(path).map_err(|e| file_error(path, e))?;
    let file_len = file
        .metadata()
        .map_err(|e| file_error(path, e))?
        .len();
    if file_len < HEADER_LEN {
        return Err(file_error(
            path,
            std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "file is shorter than a WAV header",
            ),
        ));
    }
    let mut header = [0u8; HEADER_LEN as usize];
    file.read_exact(&mut header)
        .map_err(|e| file_error(path, e))?;
    let spec = parse_header(&header).map_err(|e| file_error(path, e))?;
    let declared = u32::from_le_bytes(header[40..44].try_into().unwrap()) as u64;
    let available = file_len - HEADER_LEN;
    let data_bytes = declared.min(available);
    Ok(WavInfo {
        path: path.to_path_buf(),
        sample_rate: spec.sample_rate,
        channels: spec.channels,
        frames: data_bytes / 2,
    })
}

fn parse_header(header: &[u8; HEADER_LEN as usize]) -> std::io::Result<WavSpec> {
    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" || &header[12..16] != b"fmt " {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "not a RIFF/WAVE file",
        ));
    }
    let fmt_len = u32::from_le_bytes(header[16..20].try_into().unwrap());
    let format = u16::from_le_bytes(header[20..22].try_into().unwrap());
    let channels = u16::from_le_bytes(header[22..24].try_into().unwrap());
    let sample_rate = u32::from_le_bytes(header[24..28].try_into().unwrap());
    let bits = u16::from_le_bytes(header[34..36].try_into().unwrap());
    if fmt_len < 16 || format != FORMAT_PCM || channels == 0 || sample_rate == 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "unsupported WAV format",
        ));
    }
    if bits != BITS_PER_SAMPLE {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("expected 16-bit PCM, found {bits}-bit"),
        ));
    }
    if &header[36..40] != b"data" {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "missing WAV data chunk",
        ));
    }
    Ok(WavSpec {
        sample_rate,
        channels,
    })
}

/// Compute a fixed number of normalized peak buckets from a 16-bit mono WAV.
pub fn read_peaks(path: &Path, buckets: usize) -> Result<Vec<u8>> {
    let info = inspect_wav(path)?;
    if buckets == 0 || info.frames == 0 {
        return Ok(Vec::new());
    }
    let mut file = BufReader::new(File::open(path).map_err(|e| file_error(path, e))?);
    file.seek(SeekFrom::Start(HEADER_LEN))
        .map_err(|e| file_error(path, e))?;

    let frames_per_bucket = (info.frames as f64 / buckets as f64).max(1.0);
    let mut peaks = Vec::with_capacity(buckets);
    let mut current_peak = 0f32;
    let mut next_boundary = frames_per_bucket;
    let mut index = 0u64;
    let mut buf = vec![0u8; 8192];

    loop {
        let read = file.read(&mut buf).map_err(|e| file_error(path, e))?;
        if read == 0 {
            break;
        }
        for chunk in buf[..read].chunks_exact(2) {
            let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
            current_peak = current_peak.max((sample as f32 / 32768.0).abs());
            index += 1;
            if index as f64 >= next_boundary {
                peaks.push(current_peak);
                current_peak = 0.0;
                next_boundary += frames_per_bucket;
            }
        }
    }
    if peaks.len() < buckets {
        peaks.push(current_peak);
    }
    Ok(normalize_peaks(&peaks, buckets))
}

pub(crate) fn normalize_peaks(peaks: &[f32], buckets: usize) -> Vec<u8> {
    if peaks.is_empty() || buckets == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(buckets);
    for bucket in 0..buckets {
        let start = bucket * peaks.len() / buckets;
        let end = ((bucket + 1) * peaks.len() / buckets).max(start + 1);
        let max = peaks[start..end.min(peaks.len())]
            .iter()
            .copied()
            .fold(0f32, f32::max)
            .min(1.0);
        out.push((max.sqrt() * 255.0).round().clamp(0.0, 255.0) as u8);
    }
    out
}

pub fn downsample_peaks(peaks: &[f32], buckets: usize) -> Vec<u8> {
    normalize_peaks(peaks, buckets)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_part(dir: &Path) -> PathBuf {
        dir.join("voice.part")
    }

    #[test]
    fn writes_valid_header_and_frames() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_part(dir.path());
        let mut writer = WavWriter::create(&path, 48_000, 1).unwrap();
        writer.write_samples(&[0, 1000, -1000, 32767]).unwrap();
        let info = writer.finalize().unwrap();
        assert_eq!(info.frames, 4);
        assert_eq!(info.sample_rate, 48_000);
        assert_eq!(info.channels, 1);
        assert_eq!(info.duration_ms(), 0);

        let bytes = std::fs::read(&info.path).unwrap();
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(
            u32::from_le_bytes(bytes[40..44].try_into().unwrap()),
            8
        );
        assert_eq!(bytes.len(), 44 + 8);
    }

    #[test]
    fn recovers_an_unfinished_recording() {
        let dir = tempfile::tempdir().unwrap();
        let path = temp_part(dir.path());
        let mut writer = WavWriter::create(&path, 44_100, 1).unwrap();
        writer.write_samples(&[10; 441]).unwrap();
        let partial = writer.finish_partial().unwrap();
        assert_eq!(partial.frames, 441);
        assert!(path.exists());

        let recovered = recover_partial(&path).unwrap().unwrap();
        assert_eq!(recovered.frames, 441);
        assert!(recovered.path.exists());
        assert!(!path.exists());
        assert_eq!(recovered.duration_ms(), 10);
    }
}
