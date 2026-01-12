//! Audio control methods for ColangScreen
//!
//! Handles audio device selection, mic monitoring, and level visualization.

use makepad_widgets::*;

use super::ColangScreen;

impl ColangScreen {
    /// Initialize audio manager and populate device dropdowns
    pub(super) fn init_audio(&mut self, cx: &mut Cx) {
        let mut audio_manager = crate::audio::AudioManager::new();

        // Get input devices
        let input_devices = audio_manager.get_input_devices();
        let input_labels: Vec<String> = input_devices.iter().map(|d| {
            if d.is_default {
                format!("{} (Default)", d.name)
            } else {
                d.name.clone()
            }
        }).collect();
        self.input_devices = input_devices.iter().map(|d| d.name.clone()).collect();

        // Get output devices
        let output_devices = audio_manager.get_output_devices();
        let output_labels: Vec<String> = output_devices.iter().map(|d| {
            if d.is_default {
                format!("{} (Default)", d.name)
            } else {
                d.name.clone()
            }
        }).collect();
        self.output_devices = output_devices.iter().map(|d| d.name.clone()).collect();

        // Populate input dropdown
        if !input_labels.is_empty() {
            let dropdown = self.view.drop_down(ids!(audio_container.device_container.device_selectors.input_device_group.input_device_dropdown));
            dropdown.set_labels(cx, input_labels);
            dropdown.set_selected_item(cx, 0);
        }

        // Populate output dropdown
        if !output_labels.is_empty() {
            let dropdown = self.view.drop_down(ids!(audio_container.device_container.device_selectors.output_device_group.output_device_dropdown));
            dropdown.set_labels(cx, output_labels);
            dropdown.set_selected_item(cx, 0);
        }

        // Start mic monitoring with default device
        if let Err(e) = audio_manager.start_mic_monitoring(None) {
            eprintln!("Failed to start mic monitoring: {}", e);
        }

        self.audio_manager = Some(audio_manager);

        // Initialize audio player for TTS playback (32kHz for PrimeSpeech)
        match crate::audio_player::create_audio_player(32000) {
            Ok(player) => {
                ::log::info!("Audio player initialized (32kHz)");
                self.audio_player = Some(player);
            }
            Err(e) => {
                ::log::error!("Failed to create audio player: {}", e);
            }
        }

        // Start timer for mic level updates (50ms for smooth visualization)
        self.audio_timer = cx.start_interval(0.05);

        // Start dora timer for participant panel updates (needed for audio visualization)
        self.dora_timer = cx.start_interval(0.1);

        // AEC enabled by default (blink animation is shader-driven, no timer needed)
        self.aec_enabled = true;

        // Initialize demo log entries
        self.init_demo_logs(cx);

        self.view.redraw(cx);
    }

    /// Initialize log entries with a startup message
    pub(super) fn init_demo_logs(&mut self, cx: &mut Cx) {
        // Start with empty logs - real logs will come from log_bridge
        self.log_entries = vec![
            "[INFO] [App] Colang initialized".to_string(),
            "[INFO] [App] System log ready - Rust logs will appear here".to_string(),
        ];

        // Update the log display
        self.update_log_display(cx);
    }

    /// Update mic level LEDs based on current audio input
    pub(super) fn update_mic_level(&mut self, cx: &mut Cx) {
        let level = if let Some(ref audio_manager) = self.audio_manager {
            audio_manager.get_mic_level()
        } else {
            return;
        };

        // Map level (0.0-1.0) to 5 LEDs
        // Use non-linear scaling for better visualization (human hearing is logarithmic)
        let scaled_level = (level * 3.0).min(1.0); // Amplify for visibility
        let active_leds = (scaled_level * 5.0).ceil() as u32;

        // Colors as vec4: green=#22c55f, yellow=#eab308, orange=#f97316, red=#ef4444, off=#e2e8f0
        let green = vec4(0.133, 0.773, 0.373, 1.0);
        let yellow = vec4(0.918, 0.702, 0.031, 1.0);
        let orange = vec4(0.976, 0.451, 0.086, 1.0);
        let red = vec4(0.937, 0.267, 0.267, 1.0);
        let off = vec4(0.886, 0.910, 0.941, 1.0);

        // LED colors by index: 0,1=green, 2=yellow, 3=orange, 4=red
        let led_colors = [green, green, yellow, orange, red];
        let led_ids = [
            ids!(audio_container.mic_container.mic_group.mic_level_meter.mic_led_1),
            ids!(audio_container.mic_container.mic_group.mic_level_meter.mic_led_2),
            ids!(audio_container.mic_container.mic_group.mic_level_meter.mic_led_3),
            ids!(audio_container.mic_container.mic_group.mic_level_meter.mic_led_4),
            ids!(audio_container.mic_container.mic_group.mic_level_meter.mic_led_5),
        ];

        for (i, led_id) in led_ids.iter().enumerate() {
            let is_active = (i + 1) as u32 <= active_leds;
            let color = if is_active { led_colors[i] } else { off };
            self.view.view(*led_id).apply_over(cx, live! {
                draw_bg: { color: (color) }
            });
        }

        self.view.redraw(cx);
    }

    /// Select input device for mic monitoring
    pub(super) fn select_input_device(&mut self, cx: &mut Cx, device_name: &str) {
        if let Some(ref mut audio_manager) = self.audio_manager {
            if let Err(e) = audio_manager.set_input_device(device_name) {
                eprintln!("Failed to set input device '{}': {}", device_name, e);
            }
        }
        self.view.redraw(cx);
    }

    /// Select output device
    pub(super) fn select_output_device(&mut self, device_name: &str) {
        if let Some(ref mut audio_manager) = self.audio_manager {
            audio_manager.set_output_device(device_name);
        }
    }
}
