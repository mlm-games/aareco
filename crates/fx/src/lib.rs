use std::path::Path;

use aareco_store::{FxBackend, FxSlot};
use anyhow::{Context, Result};
use yadaw_plugin_api::{BackendKind, HostConfig, ProcessCtx, UnifiedParamInfo};
use yadaw_plugin_host::HostFacade;

pub const BLOCK_FRAMES: usize = 512;

#[derive(Clone, Debug, PartialEq)]
pub struct EffectInfo {
    pub backend: FxBackend,
    pub uri: String,
    pub name: String,
}

pub fn scan_effect_infos(
    sample_rate: f64,
    extra_paths: &[std::path::PathBuf],
) -> Result<Vec<EffectInfo>> {
    Ok(scan_effects(sample_rate, extra_paths, true)?
        .into_iter()
        .map(effect_info)
        .collect())
}

fn effect_info(plugin: yadaw_plugin_api::UnifiedPluginInfo) -> EffectInfo {
    EffectInfo {
        backend: backend_from(plugin.backend),
        uri: plugin.uri,
        name: plugin.name,
    }
}

pub fn scan_plugin_file(sample_rate: f64, path: &Path) -> Result<Vec<EffectInfo>> {
    if !path.exists() {
        anyhow::bail!("{} does not exist", path.display());
    }
    if let Some(bundle) = vst3_bundle(path) {
        let name = bundle
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let found = scan_effects(sample_rate, std::slice::from_ref(&bundle), false)?;
        let effects: Vec<_> = found
            .into_iter()
            .filter(|plugin| {
                Path::new(&plugin.uri)
                    .file_name()
                    .is_some_and(|found| found == name.as_str())
            })
            .map(effect_info)
            .collect();
        if effects.is_empty() {
            anyhow::bail!("no effects found in {}", bundle.display());
        }
        return Ok(effects);
    }

    let directory = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("{} has no parent folder", path.display()))?;
    let prefix = format!("file://{}#", path.display());
    let found = scan_effects(sample_rate, &[directory.to_path_buf()], false)?;
    let effects: Vec<_> = found
        .into_iter()
        .filter(|plugin| plugin.uri.starts_with(&prefix))
        .map(effect_info)
        .collect();
    if effects.is_empty() {
        anyhow::bail!("no effects found in {}", path.display());
    }
    Ok(effects)
}

fn vst3_bundle(path: &Path) -> Option<std::path::PathBuf> {
    let mut current = Some(path);
    while let Some(candidate) = current {
        if candidate
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("vst3"))
        {
            return Some(candidate.to_path_buf());
        }
        current = candidate.parent();
    }
    None
}

pub fn scan_effects(
    sample_rate: f64,
    extra_paths: &[std::path::PathBuf],
    defaults: bool,
) -> Result<Vec<yadaw_plugin_api::UnifiedPluginInfo>> {
    let host = host(sample_rate, extra_paths, defaults)?;
    let mut effects: Vec<_> = host
        .scan()?
        .into_iter()
        .filter(|plugin| !plugin.is_instrument && plugin.audio_inputs > 0)
        .collect();
    effects.sort_by_key(|effect| effect.name.to_lowercase());
    Ok(effects)
}

pub fn backend_of(backend: FxBackend) -> BackendKind {
    match backend {
        FxBackend::Vst3 => BackendKind::Vst3,
        FxBackend::Lv2 => BackendKind::Lv2,
        FxBackend::Clap => BackendKind::Clap,
    }
}

pub fn backend_from(backend: BackendKind) -> FxBackend {
    match backend {
        BackendKind::Vst3 => FxBackend::Vst3,
        BackendKind::Lv2 => FxBackend::Lv2,
        BackendKind::Clap => FxBackend::Clap,
    }
}

pub fn describe(slot: &FxSlot, sample_rate: f64) -> Result<Vec<UnifiedParamInfo>> {
    let host = host(sample_rate, &[], true)?;
    let instance = host
        .instantiate(backend_of(slot.backend), &slot.uri)
        .with_context(|| format!("could not load {}", slot.name))?;
    Ok(instance.params().to_vec())
}

fn host(
    sample_rate: f64,
    extra_paths: &[std::path::PathBuf],
    defaults: bool,
) -> Result<HostFacade> {
    let mut paths = if defaults { scan_paths() } else { Vec::new() };
    for path in extra_paths {
        if !paths.contains(path) {
            paths.push(path.clone());
        }
    }
    HostFacade::new(HostConfig {
        sample_rate,
        max_block: BLOCK_FRAMES,
        plugin_scan_paths: paths,
    })
}

fn scan_paths() -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();
    if cfg!(target_os = "linux") {
        paths.push(std::path::PathBuf::from("/usr/lib/clap"));
        paths.push(std::path::PathBuf::from("/usr/local/lib/clap"));
        if let Some(home) = std::env::var_os("HOME") {
            paths.push(std::path::PathBuf::from(home).join(".clap"));
        }
    } else if cfg!(target_os = "macos") {
        paths.push(std::path::PathBuf::from("/Library/Audio/Plug-Ins/CLAP"));
        if let Some(home) = std::env::var_os("HOME") {
            paths.push(std::path::PathBuf::from(home).join("Library/Audio/Plug-Ins/CLAP"));
        }
    } else if cfg!(target_os = "windows")
        && let Some(common) = std::env::var_os("COMMONPROGRAMFILES")
    {
        paths.push(std::path::PathBuf::from(common).join("CLAP"));
    }
    paths
}

pub struct FxChain {
    sample_rate: f64,
    slots: Vec<Loaded>,
}

struct Loaded {
    instance: Box<dyn yadaw_plugin_api::PluginInstance>,
    bypass: bool,
}

impl FxChain {
    pub fn new(slots: &[FxSlot], sample_rate: f64) -> Result<Self> {
        let host = host(sample_rate, &[], false)?;
        let mut loaded = Vec::new();
        for slot in slots {
            let mut instance = host
                .instantiate(backend_of(slot.backend), &slot.uri)
                .with_context(|| format!("could not load {}", slot.name))?;
            for param in &slot.params {
                if let Some(key) = parse_param_key(&param.key) {
                    instance.set_param(&key, param.value);
                }
            }
            loaded.push(Loaded {
                instance,
                bypass: slot.bypass,
            });
        }
        Ok(Self {
            sample_rate,
            slots: loaded,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(|slot| slot.bypass)
    }

    pub fn process(&mut self, ctx: &ProcessCtx, input: &[f32], output: &mut [f32]) -> Result<()> {
        let frames = input.len().min(output.len());
        if frames == 0 {
            return Ok(());
        }

        let mut left = vec![0.0f32; frames];
        let mut right = vec![0.0f32; frames];
        left.copy_from_slice(&input[..frames]);
        right.copy_from_slice(&input[..frames]);

        for slot in &mut self.slots {
            if slot.bypass {
                continue;
            }
            let mut out_left = vec![0.0f32; frames];
            let mut out_right = vec![0.0f32; frames];
            let block_ctx = ProcessCtx { frames, ..*ctx };
            let in_channels: [&[f32]; 2] = [&left, &right];
            let mut out_channels: [&mut [f32]; 2] = [&mut out_left, &mut out_right];
            slot.instance
                .process(&block_ctx, &in_channels, &mut out_channels, &[])
                .with_context(|| "effect processing failed")?;
            left = out_left;
            right = out_right;
        }

        for frame in 0..frames {
            output[frame] = (left[frame] + right[frame]) * 0.5;
        }
        Ok(())
    }

    pub fn render_wav(&mut self, input: &Path, output: &Path) -> Result<()> {
        let temp = output.with_extension("tmp");
        let result = self
            .render_wav_to(input, &temp)
            .and_then(|()| std::fs::rename(&temp, output).map_err(Into::into));
        if result.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        result
    }

    fn render_wav_to(&mut self, input: &Path, output: &Path) -> Result<()> {
        let mut reader = hound::WavReader::open(input)
            .with_context(|| format!("could not open {}", input.display()))?;
        let spec = reader.spec();
        if spec.channels != 1 || spec.bits_per_sample != 16 {
            anyhow::bail!("only 16-bit mono recordings can be processed");
        }
        if (spec.sample_rate as f64 - self.sample_rate).abs() > 0.5 {
            anyhow::bail!("recording sample rate changed while processing");
        }
        let mut writer = hound::WavWriter::create(
            output,
            hound::WavSpec {
                channels: 1,
                sample_rate: spec.sample_rate,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )?;

        let mut total_frames = 0u64;
        let mut block = Vec::with_capacity(BLOCK_FRAMES);
        let mut samples = reader.samples::<i16>();
        loop {
            block.clear();
            for _ in 0..BLOCK_FRAMES {
                match samples.next() {
                    Some(sample) => block.push(sample? as f32 / 32768.0),
                    None => break,
                }
            }
            if block.is_empty() {
                break;
            }
            let ctx = ProcessCtx {
                frames: block.len(),
                bpm: 120.0,
                beats: total_frames as f64 / self.sample_rate * 2.0,
                time_samples: total_frames as f64,
                loop_active: false,
                playing: true,
            };
            let mut out = vec![0.0f32; block.len()];
            self.process(&ctx, &block, &mut out)?;
            for sample in &out {
                writer.write_sample((sample.clamp(-1.0, 1.0) * 32767.0).round() as i16)?;
            }
            total_frames += block.len() as u64;
        }
        writer.finalize()?;
        Ok(())
    }
}

pub fn render_wav_slot(slot: &FxSlot, input: &Path, output: &Path) -> Result<()> {
    let reader = hound::WavReader::open(input)
        .with_context(|| format!("could not open {}", input.display()))?;
    let sample_rate = reader.spec().sample_rate as f64;
    drop(reader);
    let mut chain = FxChain::new(std::slice::from_ref(slot), sample_rate)?;
    chain.render_wav(input, output)
}

fn parse_param_key(key: &str) -> Option<yadaw_plugin_api::ParamKey> {
    let (kind, value) = key.split_once(':')?;
    match kind {
        "clap" => Some(yadaw_plugin_api::ParamKey::Clap(value.parse().ok()?)),
        "lv2" => Some(yadaw_plugin_api::ParamKey::Lv2(value.to_string())),
        "vst3" => Some(yadaw_plugin_api::ParamKey::Vst3(value.parse().ok()?)),
        _ => None,
    }
}

pub fn param_key(key: &yadaw_plugin_api::ParamKey) -> String {
    match key {
        yadaw_plugin_api::ParamKey::Clap(id) => format!("clap:{id}"),
        yadaw_plugin_api::ParamKey::Lv2(uri) => format!("lv2:{uri}"),
        yadaw_plugin_api::ParamKey::Vst3(id) => format!("vst3:{id}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_a_wav_without_effects_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("in.wav");
        let output = dir.path().join("out.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let samples: Vec<i16> = (0..1000).map(|i| (i * 7) as i16).collect();
        {
            let mut writer = hound::WavWriter::create(&input, spec).unwrap();
            for sample in &samples {
                writer.write_sample(*sample).unwrap();
            }
            writer.finalize().unwrap();
        }

        let mut chain = FxChain::new(&[], 48_000.0).unwrap();
        chain.render_wav(&input, &output).unwrap();

        let mut reader = hound::WavReader::open(&output).unwrap();
        assert_eq!(reader.spec(), spec);
        let rendered: Vec<i16> = reader.samples().map(|s| s.unwrap()).collect();
        assert_eq!(rendered.len(), samples.len());
        for (rendered, original) in rendered.iter().zip(samples.iter()) {
            assert!((i32::from(*rendered) - i32::from(*original)).abs() <= 1);
        }
        assert!(!output.with_extension("tmp").exists());
    }
}
