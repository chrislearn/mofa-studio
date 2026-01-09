//! Dora integration handlers for ColangScreen
//!
//! Handles dataflow control, event processing, and participant panel updates.

use makepad_widgets::*;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::dora_integration::{DoraEvent, DoraIntegration};
use crate::mofa_hero::{ConnectionStatus, MofaHeroWidgetExt};
use mofa_settings::data::Preferences;

use super::{ChatMessageEntry, ColangScreen};

impl ColangScreen {
    // =====================================================
    // Dora Integration Methods
    // =====================================================

    /// Initialize dora integration (lazy initialization)
    pub(super) fn init_dora(&mut self, cx: &mut Cx) {
        if self.dora_integration.is_some() {
            return;
        }

        ::log::info!("Initializing Dora integration");
        let integration = DoraIntegration::new();
        self.dora_integration = Some(integration);

        // Start timer to poll for dora events (100ms interval)
        self.dora_timer = cx.start_interval(0.1);

        // Look for default dataflow relative to current working directory
        // Check multiple possible locations
        let dataflow_path = std::env::current_dir().ok().and_then(|cwd| {
            // First try: apps/colang/dataflow/learning.yml (when running from workspace root)
            let app_path = cwd
                .join("apps")
                .join("colang")
                .join("dataflow")
                .join("learning.yml");
            if app_path.exists() {
                return Some(app_path);
            }
            // Second try: dataflow/learning.yml (when running from app directory)
            let local_path = cwd.join("dataflow").join("learning.yml");
            if local_path.exists() {
                return Some(local_path);
            }
            None
        });
        self.dataflow_path = dataflow_path;

        ::log::info!(
            "Dora integration initialized, dataflow: {:?}",
            self.dataflow_path
        );
    }

    /// Start a dataflow
    pub fn start_dataflow(&mut self, cx: &mut Cx, path: impl Into<PathBuf>) {
        self.init_dora(cx);

        let path = path.into();
        if let Some(ref dora) = self.dora_integration {
            if dora.start_dataflow(&path) {
                ::log::info!("Starting dataflow: {:?}", path);
                self.dataflow_path = Some(path);
                self.add_log(cx, &format!("[INFO] [App] Starting dataflow..."));
            } else {
                ::log::error!("Failed to start dataflow: {:?}", path);
                self.add_log(cx, &format!("[ERROR] [App] Failed to start dataflow"));
            }
        }
    }

    /// Stop the current dataflow
    pub fn stop_dataflow(&mut self, cx: &mut Cx) {
        if let Some(ref dora) = self.dora_integration {
            if dora.stop_dataflow() {
                ::log::info!("Stopping dataflow");
                self.add_log(cx, "[INFO] [App] Dataflow stopped");
            }
        }
    }

    /// Poll for dora events and update UI
    pub(super) fn poll_dora_events(&mut self, cx: &mut Cx) {
        // Get dora events if integration is running
        let events = if let Some(ref dora) = self.dora_integration {
            dora.poll_events()
        } else {
            Vec::new() // Continue to update audio visualization even without dora
        };

        for event in events {
            match event {
                DoraEvent::DataflowStarted { dataflow_id } => {
                    ::log::info!("Dataflow started2: {}", dataflow_id);
                    self.add_log(
                        cx,
                        &format!("[INFO] [App] Dataflow started2: {}", dataflow_id),
                    );
                    self.view
                        .mofa_hero(ids!(left_column.mofa_hero))
                        .set_connection_status(cx, ConnectionStatus::Connected);
                }
                DoraEvent::DataflowStopped => {
                    ::log::info!("Dataflow stopped");
                    self.add_log(cx, "[INFO] [App] Dataflow stopped");
                    self.view
                        .mofa_hero(ids!(left_column.mofa_hero))
                        .set_running(cx, false);
                    self.view
                        .mofa_hero(ids!(left_column.mofa_hero))
                        .set_connection_status(cx, ConnectionStatus::Stopped);
                }
                DoraEvent::BridgeConnected { bridge_name } => {
                    ::log::info!("Bridge connected: {}", bridge_name);
                    let display_name = Self::format_bridge_name(&bridge_name);
                    self.add_log(
                        cx,
                        &format!(
                            "[INFO] [Bridge] {} connected to dora dataflow",
                            display_name
                        ),
                    );
                }
                DoraEvent::BridgeDisconnected { bridge_name } => {
                    ::log::info!("Bridge disconnected: {}", bridge_name);
                    let display_name = Self::format_bridge_name(&bridge_name);
                    self.add_log(
                        cx,
                        &format!(
                            "[WARN] [Bridge] {} disconnected from dora dataflow",
                            display_name
                        ),
                    );
                }
                DoraEvent::ChatReceived { message } => {
                    // Handle streaming message consolidation
                    // Match by BOTH sender AND session_id to avoid confusion between participants
                    let sender = message.sender.clone();
                    let session_id = message.session_id.clone();

                    // Debug logging for chat messages
                    ::log::info!("[Chat] sender={}, session_id={:?}, is_streaming={}, content_len={}, pending_count={}, finalized_count={}",
                        sender,
                        session_id,
                        message.is_streaming,
                        message.content.len(),
                        self.pending_streaming_messages.len(),
                        self.chat_messages.len()
                    );

                    if message.is_streaming {
                        // Update or create pending streaming message
                        let entry = ChatMessageEntry {
                            sender: sender.clone(),
                            content: message.content.clone(),
                            timestamp: message.timestamp,
                            is_streaming: true,
                            session_id: session_id.clone(),
                        };

                        // Find existing pending message with same sender AND session_id
                        // (or same sender if session_id is "unknown")
                        let found = self.pending_streaming_messages.iter_mut()
                            .find(|m| {
                                m.sender == sender && (
                                    // Match by session_id if both have real session_ids
                                    (m.session_id.as_ref().map(|s| s != "unknown").unwrap_or(false)
                                        && session_id.as_ref().map(|s| s != "unknown").unwrap_or(false)
                                        && m.session_id == session_id)
                                    ||
                                    // For "unknown" session_ids, just match by sender
                                    (m.session_id.as_ref().map(|s| s == "unknown").unwrap_or(true)
                                        && session_id.as_ref().map(|s| s == "unknown").unwrap_or(true))
                                )
                            });

                        if let Some(pending) = found {
                            // Update existing pending message
                            pending.content = entry.content;
                            pending.timestamp = entry.timestamp;
                            pending.session_id = entry.session_id;
                        } else {
                            // Add new pending message
                            self.pending_streaming_messages.push(entry);
                        }

                        // Update display with pending messages (shown but not finalized)
                        self.update_chat_display(cx);
                    } else {
                        // Streaming complete - finalize the message
                        let entry = ChatMessageEntry {
                            sender: sender.clone(),
                            content: message.content.clone(),
                            timestamp: message.timestamp,
                            is_streaming: false,
                            session_id: session_id.clone(),
                        };

                        // Remove from pending - match by sender AND session_id
                        self.pending_streaming_messages.retain(|m| {
                            !(m.sender == sender
                                && (m.session_id == session_id
                                    || (m
                                        .session_id
                                        .as_ref()
                                        .map(|s| s == "unknown")
                                        .unwrap_or(true)
                                        && session_id
                                            .as_ref()
                                            .map(|s| s == "unknown")
                                            .unwrap_or(true))))
                        });

                        // Add to finalized messages
                        self.chat_messages.push(entry);
                        // Keep chat messages bounded (prevents O(n²) slowdown and markdown overflow)
                        if self.chat_messages.len() > 500 {
                            self.chat_messages.remove(0);
                        }
                        self.update_chat_display(cx);
                    }
                }
                DoraEvent::LogReceived { entry } => {
                    let level_str = format!("{:?}", entry.level).to_uppercase();
                    let log_line = format!("[{}] [{}] {}", level_str, entry.node_id, entry.message);
                    self.add_log(cx, &log_line);
                }
                DoraEvent::AudioReceived { data } => {
                    // Forward to audio player for playback with question_id for smart reset
                    if let Some(ref player) = self.audio_player {
                        player.write_audio_with_question(
                            &data.samples,
                            data.participant_id.clone(),
                            data.question_id.clone(),
                        );
                    }
                }
                // NOTE: ParticipantAudioReceived removed - LED visualization calculated below
                // from output waveform (more accurate since it reflects what's actually playing)
                DoraEvent::Error { message } => {
                    ::log::error!("Dora error: {}", message);
                    self.add_log(cx, &format!("[ERROR] [Dora] {}", message));
                    self.view
                        .mofa_hero(ids!(left_column.mofa_hero))
                        .set_connection_status(cx, ConnectionStatus::Failed);
                }
            }
        }

        // Update audio buffer level in mofa_hero (from audio player)
        let (is_playing, active_idx, waveform_data) = if let Some(ref player) = self.audio_player {
            let buffer_pct = player.buffer_fill_percentage() / 100.0;
            self.view
                .mofa_hero(ids!(left_column.mofa_hero))
                .set_buffer_level(cx, buffer_pct);
            (
                player.is_playing(),
                player.current_participant_idx(),
                player.get_waveform_data(),
            )
        } else {
            (false, None, Vec::new())
        };

        {
            // Calculate band levels from waveform data (same as conference-dashboard)
            let band_levels: [f32; 8] = if waveform_data.is_empty() {
                [0.0f32; 8]
            } else {
                let samples = &waveform_data;
                let band_size = samples.len() / 8;
                let mut levels = [0.0f32; 8];
                let peak = samples
                    .iter()
                    .map(|s| s.abs())
                    .fold(0.0f32, |a, b| a.max(b));
                let norm_factor = if peak > 0.01 { 1.0 / peak } else { 1.0 };

                for i in 0..8 {
                    let start = i * band_size;
                    let end = ((i + 1) * band_size).min(samples.len());
                    if end > start {
                        let sum_sq: f32 = samples[start..end].iter().map(|s| s * s).sum();
                        let rms = (sum_sq / (end - start) as f32).sqrt();
                        levels[i] = (rms * norm_factor * 1.5).clamp(0.0, 1.0);
                    }
                }
                levels
            };

            // Update participant panels using direct apply_over (exactly like conference-dashboard)
            let panel_ids: [&[LiveId]; _] = [
                ids!(
                    left_column
                        .participant_container
                        .participant_bar
                        .myself_panel
                ),
                ids!(
                    left_column
                        .participant_container
                        .participant_bar
                        .teacher_panel
                ),
            ];

            for (i, panel_id) in panel_ids.into_iter().enumerate() {
                let panel = self.view.view(panel_id);
                let is_current_audio_speaker = is_playing && active_idx == Some(i);

                // Calculate level with decay (matches conference-dashboard)
                let new_level = if is_current_audio_speaker && !waveform_data.is_empty() {
                    let samples = &waveform_data;
                    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
                    let rms = (sum_sq / samples.len() as f32).sqrt();
                    (rms * 2.0).clamp(0.0, 1.0) as f64
                } else {
                    self.participant_levels[i] * 0.85
                };
                self.participant_levels[i] = new_level;

                // Update waveform - exactly like conference-dashboard
                let active_val = if is_current_audio_speaker { 1.0 } else { 0.0 };
                panel.view(ids!(waveform)).apply_over(cx, live! {
                    draw_bg: {
                        level: (new_level),
                        active: (active_val),
                        band0: (if is_current_audio_speaker { band_levels[0] as f64 } else { 0.0 }),
                        band1: (if is_current_audio_speaker { band_levels[1] as f64 } else { 0.0 }),
                        band2: (if is_current_audio_speaker { band_levels[2] as f64 } else { 0.0 }),
                        band3: (if is_current_audio_speaker { band_levels[3] as f64 } else { 0.0 }),
                        band4: (if is_current_audio_speaker { band_levels[4] as f64 } else { 0.0 }),
                        band5: (if is_current_audio_speaker { band_levels[5] as f64 } else { 0.0 }),
                        band6: (if is_current_audio_speaker { band_levels[6] as f64 } else { 0.0 }),
                        band7: (if is_current_audio_speaker { band_levels[7] as f64 } else { 0.0 }),
                    }
                });
            }
        }
    }

    // =====================================================
    // Helper Methods
    // =====================================================

    /// Format bridge node ID to a display-friendly name
    /// e.g., "mofa-audio-player" -> "Audio Player"
    ///       "mofa-system-log" -> "System Log"
    ///       "mofa-text-input" -> "Prompt Input"
    pub(super) fn format_bridge_name(node_id: &str) -> String {
        // Remove "mofa-" prefix if present
        let name = node_id.strip_prefix("mofa-").unwrap_or(node_id);

        // Convert kebab-case to Title Case
        name.split('-')
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().chain(chars).collect(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    // =====================================================
    // MoFA Start/Stop Handlers
    // =====================================================

    /// Handle MoFA start button click
    pub(super) fn handle_mofa_start(&mut self, cx: &mut Cx) {
        ::log::info!("MoFA Start clicked");

        // Clear chat window and system log
        self.chat_messages.clear();
        self.pending_streaming_messages.clear();
        self.last_chat_count = 0;
        self.update_chat_display(cx);
        self.clear_logs(cx);

        // Initialize dora if not already done
        self.init_dora(cx);

        // Load API keys from preferences
        let env_vars = self.load_api_keys_from_preferences();

        // Log which keys are available
        let has_openai = env_vars.contains_key("OPENAI_API_KEY");
        let has_deepseek = env_vars.contains_key("DEEPSEEK_API_KEY");
        self.add_log(
            cx,
            &format!(
                "[INFO] [App] API Keys: OpenAI={}, DeepSeek={}",
                if has_openai { "✓" } else { "✗" },
                if has_deepseek { "✓" } else { "✗" }
            ),
        );

        // Find the dataflow file relative to current working directory
        let Some(dataflow_path) = self.dataflow_path.clone() else {
            self.add_log(cx, &format!("[ERROR] [App] Dataflow path not set"));
            return;
        };

        if !dataflow_path.exists() {
            self.add_log(
                cx,
                &format!("[ERROR] [App] Dataflow not found: {:?}", dataflow_path),
            );
            self.view
                .mofa_hero(ids!(left_column.mofa_hero))
                .set_connection_status(cx, ConnectionStatus::Failed);
            return;
        }

        self.add_log(
            cx,
            &format!("[INFO] [App] Starting dataflow: {:?}", dataflow_path),
        );

        // Update UI state - show connecting
        self.view
            .mofa_hero(ids!(left_column.mofa_hero))
            .set_running(cx, true);
        self.view
            .mofa_hero(ids!(left_column.mofa_hero))
            .set_connection_status(cx, ConnectionStatus::Connecting);

        // Start dataflow with environment variables
        if let Some(ref dora) = self.dora_integration {
            if !dora.start_dataflow_with_env(&dataflow_path, env_vars) {
                self.add_log(cx, "[ERROR] [App] Failed to send start command");
                self.view
                    .mofa_hero(ids!(left_column.mofa_hero))
                    .set_connection_status(cx, ConnectionStatus::Failed);
            }
        }

        // self.dataflow_path = Some(dataflow_path);
    }

    /// Handle MoFA stop button click
    pub(super) fn handle_mofa_stop(&mut self, cx: &mut Cx) {
        ::log::info!("MoFA Stop clicked");

        self.add_log(cx, "[INFO] [App] Force stopping Colang dataflow...");

        // Show "Stopping" state while stop is in progress
        self.view
            .mofa_hero(ids!(left_column.mofa_hero))
            .set_connection_status(cx, ConnectionStatus::Stopping);

        // Force stop dataflow immediately (0s grace period)
        // The actual status update will come from DoraEvent::DataflowStopped
        if let Some(ref dora) = self.dora_integration {
            dora.force_stop_dataflow();
        }

        // Note: Don't set Stopped here - wait for DoraEvent::DataflowStopped
        // to confirm the dataflow actually stopped
    }

    /// Load API keys from preferences
    pub(super) fn load_api_keys_from_preferences(&self) -> HashMap<String, String> {
        let mut env_vars = HashMap::new();

        // Load preferences
        let prefs = Preferences::load();

        // Get OpenAI API key
        if let Some(provider) = prefs.get_provider("openai") {
            if let Some(ref api_key) = provider.api_key {
                if !api_key.is_empty() {
                    env_vars.insert("OPENAI_API_KEY".to_string(), api_key.clone());
                }
            }
        }

        // Get DeepSeek API key
        if let Some(provider) = prefs.get_provider("deepseek") {
            if let Some(ref api_key) = provider.api_key {
                if !api_key.is_empty() {
                    env_vars.insert("DEEPSEEK_API_KEY".to_string(), api_key.clone());
                }
            }
        }

        // Get Alibaba Cloud API key
        if let Some(provider) = prefs.get_provider("alibaba_cloud") {
            if let Some(ref api_key) = provider.api_key {
                if !api_key.is_empty() {
                    env_vars.insert("DASHSCOPE_API_KEY".to_string(), api_key.clone());
                }
            }
        }

        env_vars
    }
}
