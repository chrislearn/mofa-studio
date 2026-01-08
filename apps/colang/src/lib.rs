//! MoFA FM App - AI-powered audio streaming and voice interface

pub mod screen;
pub mod mofa_hero;
pub mod audio;
pub mod audio_player;
pub mod dora_integration;
pub mod log_bridge;
pub mod database;
pub mod doubao_api;

pub use screen::ColangScreen;
pub use screen::ColangScreenWidgetRefExt;  // Export WidgetRefExt for timer control
pub use mofa_hero::{MofaHero, MofaHeroAction, ConnectionStatus};
pub use audio::AudioManager;
pub use dora_integration::{DoraIntegration, DoraCommand, DoraEvent, DoraState};
pub use database::Database;
pub use doubao_api::DoubaoClient;

use makepad_widgets::Cx;
use mofa_widgets::{MofaApp, AppInfo};

/// MoFA FM app descriptor
pub struct ColangApp;

impl MofaApp for ColangApp {
    fn info() -> AppInfo {
        AppInfo {
            name: "MoFA FM",
            id: "mofa-fm",
            description: "AI-powered audio streaming and voice interface",
        }
    }

    fn live_design(cx: &mut Cx) {
        mofa_hero::live_design(cx);
        screen::live_design(cx);
    }
}

/// Register all MoFA FM widgets with Makepad
/// (Kept for backwards compatibility - calls DoraApp::live_design)
pub fn live_design(cx: &mut Cx) {
    ColangApp::live_design(cx);
}
