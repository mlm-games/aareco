#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamKind {
    Bool,
    Int,
    Float,
    Enum,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParamInfo {
    pub key: String,
    pub name: String,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub unit: Option<String>,
    pub kind: ParamKind,
}

pub const KINDS: [&str; 6] = ["highpass", "eq", "compressor", "deesser", "limiter", "gate"];

pub fn display_name(kind: &str) -> &'static str {
    match kind {
        "highpass" => "High-pass filter",
        "eq" => "Voice EQ",
        "compressor" => "Compressor",
        "deesser" => "De-esser",
        "limiter" => "Limiter",
        "gate" => "Noise gate",
        _ => "Unknown effect",
    }
}

pub fn params(kind: &str) -> Vec<ParamInfo> {
    let param = |key: &str, name: &str, min: f32, max: f32, default: f32, unit: &str| ParamInfo {
        key: key.to_string(),
        name: name.to_string(),
        min,
        max,
        default,
        unit: if unit.is_empty() {
            None
        } else {
            Some(unit.to_string())
        },
        kind: ParamKind::Float,
    };
    match kind {
        "highpass" => vec![param("cutoff", "Cutoff", 20.0, 300.0, 80.0, "Hz")],
        "eq" => vec![
            param("mud", "Mud cut (300 Hz)", -12.0, 12.0, -3.0, "dB"),
            param("presence", "Presence (3.5 kHz)", -12.0, 12.0, 3.0, "dB"),
            param("air", "Air (9 kHz)", -12.0, 12.0, 1.5, "dB"),
        ],
        "compressor" => vec![
            param("threshold", "Threshold", -40.0, 0.0, -18.0, "dB"),
            param("ratio", "Ratio", 1.0, 12.0, 3.0, ":1"),
            param("attack", "Attack", 1.0, 100.0, 10.0, "ms"),
            param("release", "Release", 20.0, 500.0, 120.0, "ms"),
            param("makeup", "Makeup", 0.0, 12.0, 3.0, "dB"),
        ],
        "deesser" => vec![
            param("frequency", "Frequency", 3000.0, 10000.0, 6000.0, "Hz"),
            param("threshold", "Threshold", -40.0, 0.0, -24.0, "dB"),
            param("amount", "Amount", 0.0, 1.0, 0.6, ""),
        ],
        "limiter" => vec![
            param("ceiling", "Ceiling", -6.0, 0.0, -1.0, "dB"),
            param("release", "Release", 10.0, 300.0, 50.0, "ms"),
        ],
        "gate" => vec![
            param("threshold", "Threshold", -60.0, 0.0, -40.0, "dB"),
            param("release", "Release", 20.0, 500.0, 120.0, "ms"),
        ],
        _ => Vec::new(),
    }
}

struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Biquad {
    fn highpass(sample_rate: f32, frequency: f32) -> Self {
        let w0 = 2.0 * std::f32::consts::PI * frequency / sample_rate;
        let alpha = w0.sin() / (2.0 * 0.707);
        let cos = w0.cos();
        let a0 = 1.0 + alpha;
        Self {
            b0: ((1.0 + cos) / 2.0) / a0,
            b1: -(1.0 + cos) / a0,
            b2: ((1.0 + cos) / 2.0) / a0,
            a1: (-2.0 * cos) / a0,
            a2: (1.0 - alpha) / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    fn peaking(sample_rate: f32, frequency: f32, gain_db: f32) -> Self {
        let w0 = 2.0 * std::f32::consts::PI * frequency / sample_rate;
        let alpha = w0.sin() / (2.0 * 1.0);
        let cos = w0.cos();
        let a = 10.0f32.powf(gain_db / 40.0);
        let a0 = 1.0 + alpha / a;
        Self {
            b0: (1.0 + alpha * a) / a0,
            b1: (-2.0 * cos) / a0,
            b2: (1.0 - alpha * a) / a0,
            a1: (-2.0 * cos) / a0,
            a2: (1.0 - alpha / a) / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    fn bandpass(sample_rate: f32, frequency: f32, q: f32) -> Self {
        let w0 = 2.0 * std::f32::consts::PI * frequency / sample_rate;
        let alpha = w0.sin() / (2.0 * q);
        let cos = w0.cos();
        let a0 = 1.0 + alpha;
        Self {
            b0: alpha / a0,
            b1: 0.0,
            b2: -alpha / a0,
            a1: (-2.0 * cos) / a0,
            a2: (1.0 - alpha) / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

fn coefficient(milliseconds: f32, sample_rate: f32) -> f32 {
    (-1.0 / (milliseconds * 0.001 * sample_rate)).exp()
}

fn to_db(value: f32) -> f32 {
    20.0 * value.max(1e-9).log10()
}

fn from_db(db: f32) -> f32 {
    10.0f32.powf(db / 20.0)
}

pub struct BuiltinEffect {
    kind: String,
    values: Vec<f32>,
    sample_rate: f32,
    filters: Vec<Biquad>,
    envelope: f32,
    reduction: f32,
    gain: f32,
}

impl BuiltinEffect {
    pub fn new(kind: &str, stored: &[(String, f32)], sample_rate: f64) -> Result<Self, String> {
        let specs = params(kind);
        if specs.is_empty() {
            return Err(format!("unknown built-in effect {kind:?}"));
        }
        let values: Vec<f32> = specs
            .iter()
            .map(|spec| {
                let normalized = stored
                    .iter()
                    .find(|(key, _)| key == &spec.key)
                    .map(|(_, value)| *value)
                    .unwrap_or((spec.default - spec.min) / (spec.max - spec.min));
                spec.min + (spec.max - spec.min) * normalized.clamp(0.0, 1.0)
            })
            .collect();
        let sample_rate = sample_rate as f32;
        let filters = match kind {
            "highpass" => vec![Biquad::highpass(sample_rate, values[0])],
            "eq" => vec![
                Biquad::peaking(sample_rate, 300.0, values[0]),
                Biquad::peaking(sample_rate, 3500.0, values[1]),
                Biquad::peaking(sample_rate, 9000.0, values[2]),
            ],
            "deesser" => vec![Biquad::bandpass(sample_rate, values[0], 1.5)],
            _ => Vec::new(),
        };
        Ok(Self {
            kind: kind.to_string(),
            values,
            sample_rate,
            filters,
            envelope: 0.0,
            reduction: 0.0,
            gain: 1.0,
        })
    }

    pub fn process(&mut self, samples: &mut [f32]) {
        match self.kind.as_str() {
            "highpass" | "eq" => {
                for sample in samples.iter_mut() {
                    let mut value = *sample;
                    for filter in &mut self.filters {
                        value = filter.process(value);
                    }
                    *sample = value;
                }
            }
            "compressor" => self.process_compressor(samples),
            "deesser" => self.process_deesser(samples),
            "limiter" => self.process_limiter(samples),
            "gate" => self.process_gate(samples),
            _ => {}
        }
    }

    fn process_compressor(&mut self, samples: &mut [f32]) {
        let threshold = self.values[0];
        let ratio = self.values[1];
        let attack = coefficient(self.values[2], self.sample_rate);
        let release = coefficient(self.values[3], self.sample_rate);
        let makeup = self.values[4];
        for sample in samples.iter_mut() {
            let peak = sample.abs();
            self.envelope = if peak > self.envelope {
                self.envelope + attack * (peak - self.envelope)
            } else {
                self.envelope + release * (peak - self.envelope)
            };
            let level = to_db(self.envelope);
            let target = if level > threshold {
                (level - threshold) * (1.0 - 1.0 / ratio)
            } else {
                0.0
            };
            let speed = if target > self.reduction {
                attack
            } else {
                release
            };
            self.reduction += speed * (target - self.reduction);
            *sample *= from_db(makeup - self.reduction);
        }
    }

    fn process_deesser(&mut self, samples: &mut [f32]) {
        let threshold = self.values[1];
        let amount = self.values[2];
        let attack = coefficient(5.0, self.sample_rate);
        let release = coefficient(100.0, self.sample_rate);
        for sample in samples.iter_mut() {
            let detected = self.filters[0].process(*sample).abs();
            self.envelope = if detected > self.envelope {
                self.envelope + attack * (detected - self.envelope)
            } else {
                self.envelope + release * (detected - self.envelope)
            };
            let level = to_db(self.envelope);
            let target = if level > threshold {
                (level - threshold) * amount
            } else {
                0.0
            };
            let speed = if target > self.reduction {
                attack
            } else {
                release
            };
            self.reduction += speed * (target - self.reduction);
            *sample *= from_db(-self.reduction);
        }
    }

    fn process_limiter(&mut self, samples: &mut [f32]) {
        let ceiling = from_db(self.values[0]);
        let release = coefficient(self.values[1], self.sample_rate);
        for sample in samples.iter_mut() {
            let peak = sample.abs();
            self.envelope = peak;
            let target = if peak > ceiling { ceiling / peak } else { 1.0 };
            self.gain = if target < self.gain {
                target
            } else {
                self.gain + release * (target - self.gain)
            };
            *sample *= self.gain;
        }
    }

    fn process_gate(&mut self, samples: &mut [f32]) {
        let threshold = from_db(self.values[0]);
        let release = coefficient(self.values[1], self.sample_rate);
        let attack = coefficient(5.0, self.sample_rate);
        for sample in samples.iter_mut() {
            let peak = sample.abs();
            self.envelope = if peak > self.envelope {
                peak
            } else {
                self.envelope + release * (peak - self.envelope)
            };
            let target = if self.envelope > threshold { 1.0 } else { 0.0 };
            let speed = if target > self.gain { attack } else { release };
            self.gain += speed * (target - self.gain);
            *sample *= self.gain;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(sample_rate: f32, frequency: f32, seconds: f32) -> Vec<f32> {
        (0..(sample_rate * seconds) as usize)
            .map(|i| (2.0 * std::f32::consts::PI * frequency * i as f32 / sample_rate).sin() * 0.5)
            .collect()
    }

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt()
    }

    #[test]
    fn highpass_removes_rumble_but_keeps_voice() {
        let mut low = sine(48_000.0, 20.0, 0.2);
        let specs = params("highpass");
        let cutoff =
            ((specs[0].default - specs[0].min) / (specs[0].max - specs[0].min)).clamp(0.0, 1.0);
        BuiltinEffect::new("highpass", &[("cutoff".to_string(), cutoff)], 48_000.0)
            .unwrap()
            .process(&mut low);
        let mut high = sine(48_000.0, 1000.0, 0.2);
        BuiltinEffect::new("highpass", &[("cutoff".to_string(), cutoff)], 48_000.0)
            .unwrap()
            .process(&mut high);
        assert!(rms(&low) < 0.05);
        assert!(rms(&high) > 0.34);
    }

    #[test]
    fn limiter_never_exceeds_ceiling() {
        let mut samples = vec![0.0f32; 4096];
        for (index, sample) in samples.iter_mut().enumerate() {
            *sample = (index as f32 * 0.3).sin() * 2.0;
        }
        let specs = params("limiter");
        let ceiling =
            ((specs[0].default - specs[0].min) / (specs[0].max - specs[0].min)).clamp(0.0, 1.0);
        let release =
            ((specs[1].default - specs[1].min) / (specs[1].max - specs[1].min)).clamp(0.0, 1.0);
        BuiltinEffect::new(
            "limiter",
            &[
                ("ceiling".to_string(), ceiling),
                ("release".to_string(), release),
            ],
            48_000.0,
        )
        .unwrap()
        .process(&mut samples);
        assert!(samples.iter().all(|sample| sample.abs() <= 0.9));
    }

    #[test]
    fn compressor_reduces_loud_input_more_than_quiet_input() {
        let specs = params("compressor");
        let normalized: Vec<(String, f32)> = specs
            .iter()
            .map(|spec| {
                (
                    spec.key.clone(),
                    ((spec.default - spec.min) / (spec.max - spec.min)).clamp(0.0, 1.0),
                )
            })
            .collect();
        let quiet = sine(48_000.0, 1000.0, 0.2)
            .iter()
            .map(|s| s * 0.05)
            .collect::<Vec<_>>();
        let loud = sine(48_000.0, 1000.0, 0.2)
            .iter()
            .map(|s| s * 0.8)
            .collect::<Vec<_>>();
        let mut quiet_out = quiet.clone();
        let mut loud_out = loud.clone();
        BuiltinEffect::new("compressor", &normalized, 48_000.0)
            .unwrap()
            .process(&mut quiet_out);
        BuiltinEffect::new("compressor", &normalized, 48_000.0)
            .unwrap()
            .process(&mut loud_out);
        let quiet_gain = rms(&quiet_out) / rms(&quiet);
        let loud_gain = rms(&loud_out) / rms(&loud);
        assert!(quiet_gain > loud_gain);
    }
}
