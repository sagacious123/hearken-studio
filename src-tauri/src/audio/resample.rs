use anyhow::Result;
use rubato::{FftFixedIn, Resampler};

use super::SAMPLE_RATE;

/// Streaming mono resampler to 16 kHz. Pass-through when the device already runs at 16 kHz.
pub struct Resampler16k {
    inner: Option<FftFixedIn<f32>>,
    pending: Vec<f32>,
}

impl Resampler16k {
    pub fn new(input_rate: u32) -> Result<Self> {
        let inner = if input_rate as usize == SAMPLE_RATE {
            None
        } else {
            // 1024-frame chunks ≈ 21 ms at 48 kHz: small enough to keep latency low.
            Some(FftFixedIn::<f32>::new(input_rate as usize, SAMPLE_RATE, 1024, 2, 1)?)
        };
        Ok(Self { inner, pending: Vec::with_capacity(4096) })
    }

    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) -> Result<()> {
        let Self { inner, pending } = self;
        let Some(resampler) = inner else {
            out.extend_from_slice(input);
            return Ok(());
        };
        pending.extend_from_slice(input);
        let mut consumed = 0;
        loop {
            let need = resampler.input_frames_next();
            if pending.len() - consumed < need {
                break;
            }
            let chunk = &pending[consumed..consumed + need];
            let res = resampler.process(&[chunk], None)?;
            out.extend_from_slice(&res[0]);
            consumed += need;
        }
        pending.drain(..consumed);
        Ok(())
    }
}
