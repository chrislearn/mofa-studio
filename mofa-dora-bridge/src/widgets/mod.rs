//! Widget-specific bridge implementations
//!
//! Each widget type has its own bridge that connects to dora as a dynamic node:
//! - `mofa-audio-player`: Receives audio, forwards to UI for playback
//! - `mofa-system-log`: Receives logs from multiple nodes
//! - `mofa-prompt-input`: Sends user prompts to LLM
//!
//! Note: LED visualization is calculated in screen.rs from output waveform
//! (more accurate since it reflects what's actually being played)

mod audio_player;
mod mic_input;
mod prompt_input;
mod system_log;
mod text_input;

pub use audio_player::AudioPlayerBridge;
pub use mic_input::MicInputBridge;
pub use prompt_input::PromptInputBridge;
pub use system_log::SystemLogBridge;
pub use text_input::TextInputBridge;
