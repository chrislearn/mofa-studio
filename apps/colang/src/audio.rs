//! Audio device management and mic level monitoring

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Host, Stream, StreamConfig};
use crossbeam_channel::{Sender, Receiver, unbounded};
use parking_lot::Mutex;
use std::sync::Arc;

/// Audio device info
#[derive(Clone, Debug)]
pub struct AudioDeviceInfo {
    pub name: String,
    pub is_default: bool,
}

/// Shared state for mic level
pub struct MicLevelState {
    pub level: f32,  // 0.0 - 1.0
    pub peak: f32,
}

impl Default for MicLevelState {
    fn default() -> Self {
        Self { level: 0.0, peak: 0.0 }
    }
}

/// Audio chunk for sending to dora
#[derive(Clone, Debug)]
pub struct AudioChunk {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

/// Audio manager for device enumeration and mic monitoring
pub struct AudioManager {
    host: Host,
    input_stream: Option<Stream>,
    mic_level: Arc<Mutex<MicLevelState>>,
    current_input_device: Option<String>,
    current_output_device: Option<String>,
    /// Channel for sending captured audio chunks
    audio_tx: Option<Sender<AudioChunk>>,
    audio_rx: Option<Receiver<AudioChunk>>,
}

impl AudioManager {
    pub fn new() -> Self {
        let host = cpal::default_host();
        let (audio_tx, audio_rx) = unbounded();
        Self {
            host,
            input_stream: None,
            mic_level: Arc::new(Mutex::new(MicLevelState::default())),
            current_input_device: None,
            current_output_device: None,
            audio_tx: Some(audio_tx),
            audio_rx: Some(audio_rx),
        }
    }

    /// Get list of input devices
    pub fn get_input_devices(&self) -> Vec<AudioDeviceInfo> {
        let default_name = self.host
            .default_input_device()
            .and_then(|d| d.name().ok());

        let mut devices = Vec::new();

        if let Ok(input_devices) = self.host.input_devices() {
            for device in input_devices {
                if let Ok(name) = device.name() {
                    let is_default = default_name.as_ref().map_or(false, |d| d == &name);
                    devices.push(AudioDeviceInfo { name, is_default });
                }
            }
        }

        // Sort with default first
        devices.sort_by(|a, b| b.is_default.cmp(&a.is_default));
        devices
    }

    /// Get list of output devices
    pub fn get_output_devices(&self) -> Vec<AudioDeviceInfo> {
        let default_name = self.host
            .default_output_device()
            .and_then(|d| d.name().ok());

        let mut devices = Vec::new();

        if let Ok(output_devices) = self.host.output_devices() {
            for device in output_devices {
                if let Ok(name) = device.name() {
                    let is_default = default_name.as_ref().map_or(false, |d| d == &name);
                    devices.push(AudioDeviceInfo { name, is_default });
                }
            }
        }

        devices.sort_by(|a, b| b.is_default.cmp(&a.is_default));
        devices
    }

    /// Find input device by name
    fn find_input_device(&self, name: &str) -> Option<Device> {
        if let Ok(devices) = self.host.input_devices() {
            for device in devices {
                if let Ok(device_name) = device.name() {
                    if device_name == name {
                        return Some(device);
                    }
                }
            }
        }
        None
    }

    /// Start monitoring mic level for a specific device
    pub fn start_mic_monitoring(&mut self, device_name: Option<&str>) -> Result<(), String> {
        // Stop existing stream
        self.stop_mic_monitoring();

        // Get device
        let device = if let Some(name) = device_name {
            self.find_input_device(name)
                .ok_or_else(|| format!("Device not found: {}", name))?
        } else {
            self.host.default_input_device()
                .ok_or_else(|| "No default input device".to_string())?
        };

        let device_name = device.name().unwrap_or_default();
        self.current_input_device = Some(device_name);

        // Get config
        let config = device.default_input_config()
            .map_err(|e| format!("Failed to get config: {}", e))?;

        let sample_format = config.sample_format();
        let stream_config: StreamConfig = config.clone().into();
        let actual_sample_rate = stream_config.sample_rate.0;

        let mic_level = self.mic_level.clone();
        let audio_tx = self.audio_tx.clone();

        // Buffer for accumulating samples before sending (send every 100ms)
        let chunk_size = (actual_sample_rate as f32 * 0.1) as usize; // 100ms chunks

        // Build stream based on sample format
        let stream = match sample_format {
            cpal::SampleFormat::F32 => {
                let buffer = Arc::new(Mutex::new(Vec::with_capacity(chunk_size)));
                let buffer_clone = buffer.clone();
                
                device.build_input_stream(
                    &stream_config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        // Update mic level for visualization
                        let mut max = 0.0f32;
                        for &sample in data {
                            let abs = sample.abs();
                            if abs > max {
                                max = abs;
                            }
                        }
                        let mut state = mic_level.lock();
                        state.level = state.level * 0.7 + max * 0.3;
                        if max > state.peak {
                            state.peak = max;
                        } else {
                            state.peak *= 0.995;
                        }
                        drop(state);

                        // Accumulate audio samples
                        if let Some(ref tx) = audio_tx {
                            let mut buf = buffer_clone.lock();
                            buf.extend_from_slice(data);
                            
                            // Send chunk when buffer is full
                            if buf.len() >= chunk_size {
                                let chunk = AudioChunk {
                                    samples: buf.clone(),
                                    sample_rate: actual_sample_rate,
                                };
                                let _ = tx.try_send(chunk);
                                buf.clear();
                            }
                        }
                    },
                    |err| eprintln!("Audio input error: {}", err),
                    None,
                )
            }
            cpal::SampleFormat::I16 => {
                let buffer = Arc::new(Mutex::new(Vec::with_capacity(chunk_size)));
                let buffer_clone = buffer.clone();
                
                device.build_input_stream(
                    &stream_config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        // Update mic level
                        let mut max = 0.0f32;
                        for &sample in data {
                            let abs = (sample as f32 / i16::MAX as f32).abs();
                            if abs > max {
                                max = abs;
                            }
                        }
                        let mut state = mic_level.lock();
                        state.level = state.level * 0.7 + max * 0.3;
                        if max > state.peak {
                            state.peak = max;
                        } else {
                            state.peak *= 0.995;
                        }
                        drop(state);

                        // Convert i16 to f32 and accumulate
                        if let Some(ref tx) = audio_tx {
                            let mut buf = buffer_clone.lock();
                            for &sample in data {
                                buf.push(sample as f32 / 32768.0);
                            }
                            
                            // Send chunk when buffer is full
                            if buf.len() >= chunk_size {
                                let chunk = AudioChunk {
                                    samples: buf.clone(),
                                    sample_rate: actual_sample_rate,
                                };
                                let _ = tx.try_send(chunk);
                                buf.clear();
                            }
                        }
                    },
                    |err| eprintln!("Audio input error: {}", err),
                    None,
                )
            }
            _ => return Err("Unsupported sample format".to_string()),
        }.map_err(|e| format!("Failed to build stream: {}", e))?;

        stream.play().map_err(|e| format!("Failed to play stream: {}", e))?;
        self.input_stream = Some(stream);

        Ok(())
    }

    /// Stop mic monitoring
    pub fn stop_mic_monitoring(&mut self) {
        self.input_stream = None;
        let mut state = self.mic_level.lock();
        state.level = 0.0;
        state.peak = 0.0;
    }

    /// Get current mic level (0.0 - 1.0)
    pub fn get_mic_level(&self) -> f32 {
        self.mic_level.lock().level
    }

    /// Set current input device
    pub fn set_input_device(&mut self, name: &str) -> Result<(), String> {
        self.start_mic_monitoring(Some(name))
    }

    /// Set current output device
    pub fn set_output_device(&mut self, name: &str) {
        self.current_output_device = Some(name.to_string());
        // Note: Output device selection would be used when playing audio
    }

    /// Get current input device name
    pub fn current_input_device(&self) -> Option<&str> {
        self.current_input_device.as_deref()
    }

    /// Get current output device name
    pub fn current_output_device(&self) -> Option<&str> {
        self.current_output_device.as_deref()
    }

    /// Poll for captured audio chunks (non-blocking)
    /// Returns all available audio chunks
    pub fn poll_audio_chunks(&self) -> Vec<AudioChunk> {
        let mut chunks = Vec::new();
        if let Some(ref rx) = self.audio_rx {
            while let Ok(chunk) = rx.try_recv() {
                chunks.push(chunk);
            }
        }
        chunks
    }
}

impl Default for AudioManager {
    fn default() -> Self {
        Self::new()
    }
}
