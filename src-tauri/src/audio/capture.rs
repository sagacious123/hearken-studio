use std::{
    sync::mpsc,
    thread::{self, JoinHandle},
};

use anyhow::{anyhow, bail, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::{traits::Producer, HeapProd};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    /// `in:<name>` for capture devices, `loopback:<name>` for system-audio loopback (Windows/WASAPI).
    pub id: String,
    pub name: String,
    pub kind: &'static str,
    pub is_default: bool,
}

pub fn list_devices() -> Vec<DeviceInfo> {
    let host = cpal::default_host();
    let default_in = host.default_input_device().and_then(|d| d.name().ok());
    let mut out = Vec::new();

    if let Ok(devices) = host.input_devices() {
        for d in devices {
            if let Ok(name) = d.name() {
                out.push(DeviceInfo {
                    id: format!("in:{name}"),
                    is_default: default_in.as_ref() == Some(&name),
                    name,
                    kind: "input",
                });
            }
        }
    }

    // WASAPI can open an *output* device as an input stream (loopback), which captures
    // whatever the PC is playing. macOS needs a virtual device (e.g. BlackHole) instead;
    // on Linux, PulseAudio/PipeWire "Monitor of …" sources already appear as inputs.
    #[cfg(target_os = "windows")]
    if let Ok(devices) = host.output_devices() {
        for d in devices {
            if let Ok(name) = d.name() {
                out.push(DeviceInfo { id: format!("loopback:{name}"), name, kind: "loopback", is_default: false });
            }
        }
    }

    out
}

/// Owns the capture thread. cpal's `Stream` is `!Send` on some platforms, so it is created,
/// played and dropped entirely on a dedicated thread; dropping this handle stops it.
pub struct CaptureHandle {
    stop_tx: Option<mpsc::Sender<()>>,
    thread: Option<JoinHandle<()>>,
    pub sample_rate: u32,
    pub device_name: String,
}

impl Drop for CaptureHandle {
    fn drop(&mut self) {
        drop(self.stop_tx.take());
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

pub fn start(device_id: Option<&str>, producer: HeapProd<f32>) -> Result<CaptureHandle> {
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(u32, String)>>();
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let device_id = device_id.map(str::to_owned);

    let thread = thread::Builder::new().name("audio-capture".into()).spawn(move || {
        match open_stream(device_id.as_deref(), producer) {
            Ok((stream, rate, name)) => {
                let _ = ready_tx.send(Ok((rate, name)));
                // Park until the handle is dropped (sender disconnects).
                let _ = stop_rx.recv();
                drop(stream);
            }
            Err(e) => {
                let _ = ready_tx.send(Err(e));
            }
        }
    })?;

    let (sample_rate, device_name) = ready_rx.recv().map_err(|_| anyhow!("audio capture thread exited"))??;
    log::info!("capturing from '{device_name}' at {sample_rate} Hz");
    Ok(CaptureHandle { stop_tx: Some(stop_tx), thread: Some(thread), sample_rate, device_name })
}

fn find_device(host: &cpal::Host, id: Option<&str>) -> Result<(cpal::Device, bool)> {
    let Some(id) = id else {
        let d = host.default_input_device().context("no default input device")?;
        return Ok((d, false));
    };
    let (loopback, name) = match id.split_once(':') {
        Some(("loopback", n)) => (true, n),
        Some(("in", n)) => (false, n),
        _ => (false, id),
    };
    let mut devices = if loopback { host.output_devices()? } else { host.input_devices()? };
    let device = devices
        .find(|d| d.name().map(|n| n == name).unwrap_or(false))
        .with_context(|| format!("audio device '{name}' not found"))?;
    Ok((device, loopback))
}

fn open_stream(id: Option<&str>, producer: HeapProd<f32>) -> Result<(cpal::Stream, u32, String)> {
    let host = cpal::default_host();
    let (device, loopback) = find_device(&host, id)?;
    let name = device.name().unwrap_or_else(|_| "unknown".into());
    let supported = if loopback { device.default_output_config()? } else { device.default_input_config()? };
    let rate = supported.sample_rate().0;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();

    let stream = match format {
        cpal::SampleFormat::F32 => build::<f32>(&device, &config, producer)?,
        cpal::SampleFormat::I16 => build::<i16>(&device, &config, producer)?,
        cpal::SampleFormat::U16 => build::<u16>(&device, &config, producer)?,
        cpal::SampleFormat::I32 => build::<i32>(&device, &config, producer)?,
        other => bail!("unsupported sample format {other:?}"),
    };
    stream.play()?;
    Ok((stream, rate, name))
}

fn build<T>(device: &cpal::Device, config: &cpal::StreamConfig, mut producer: HeapProd<f32>) -> Result<cpal::Stream>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let channels = config.channels as usize;
    let stream = device.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            // Real-time thread: downmix to mono and push; drop samples if the DSP thread stalls.
            for frame in data.chunks(channels) {
                let sum: f32 = frame.iter().map(|s| s.to_sample::<f32>()).sum();
                let _ = producer.try_push(sum / channels as f32);
            }
        },
        |err| log::error!("audio stream error: {err}"),
        None,
    )?;
    Ok(stream)
}

