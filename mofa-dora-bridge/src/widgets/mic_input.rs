//! Mic input bridge
//!
//! Connects to dora as `mofa-mic-input` dynamic node.
//! Receives audio from UI's microphone and sends to ASR nodes.

use crate::bridge::{BridgeEvent, BridgeState, DoraBridge};
use crate::data::{AudioData, DoraData, EventMetadata};
use crate::error::{BridgeError, BridgeResult};
use crossbeam_channel::{bounded, Receiver, Sender};
use dora_node_api::{
    dora_core::config::{DataId, NodeId},
    DoraNode, Event, IntoArrow, Parameter,
};
use parking_lot::RwLock;
use std::sync::Arc;
use std::thread;
use tracing::{debug, error, info, warn};

/// Mic input bridge - receives audio from UI, sends to dora
pub struct MicInputBridge {
    /// Node ID (e.g., "mofa-mic-input")
    node_id: String,
    /// Current state
    state: Arc<RwLock<BridgeState>>,
    /// Event sender to widget
    event_sender: Sender<BridgeEvent>,
    /// Event receiver for widget
    event_receiver: Receiver<BridgeEvent>,
    /// Audio data sender from widget
    audio_sender: Sender<AudioData>,
    /// Audio data receiver for dora
    audio_receiver: Receiver<AudioData>,
    /// Stop signal
    stop_sender: Option<Sender<()>>,
    /// Worker thread handle
    worker_handle: Option<thread::JoinHandle<()>>,
}

impl MicInputBridge {
    /// Create a new mic input bridge
    pub fn new(node_id: &str) -> Self {
        let (event_tx, event_rx) = bounded(100);
        let (audio_tx, audio_rx) = bounded(500); // Large buffer for audio chunks

        Self {
            node_id: node_id.to_string(),
            state: Arc::new(RwLock::new(BridgeState::Disconnected)),
            event_sender: event_tx,
            event_receiver: event_rx,
            audio_sender: audio_tx,
            audio_receiver: audio_rx,
            stop_sender: None,
            worker_handle: None,
        }
    }

    /// Send audio data to dora (widget calls this)
    pub fn send_audio(&self, audio_data: AudioData) -> BridgeResult<()> {
        self.audio_sender
            .try_send(audio_data)
            .map_err(|_| BridgeError::ChannelSendError)
    }

    /// Run the dora event loop in background thread
    fn run_event_loop(
        node_id: String,
        state: Arc<RwLock<BridgeState>>,
        event_sender: Sender<BridgeEvent>,
        audio_receiver: Receiver<AudioData>,
        stop_receiver: Receiver<()>,
    ) {
        info!("Starting mic input bridge event loop for {}", node_id);

        // Initialize dora node
        let (mut node, mut events) =
            match DoraNode::init_from_node_id(NodeId::from(node_id.clone())) {
                Ok(n) => n,
                Err(e) => {
                    error!("Failed to init dora node {}: {}", node_id, e);
                    *state.write() = BridgeState::Error;
                    let _ = event_sender.send(BridgeEvent::Error(format!("Init failed: {}", e)));
                    return;
                }
            };

        *state.write() = BridgeState::Connected;
        let _ = event_sender.send(BridgeEvent::Connected);

        // Event loop
        loop {
            // Check for stop signal
            if stop_receiver.try_recv().is_ok() {
                info!("Mic input bridge received stop signal");
                break;
            }

            // Send audio data from widget to dora
            while let Ok(audio_data) = audio_receiver.try_recv() {
                debug!(
                    "Sending audio: {} samples at {}Hz",
                    audio_data.samples.len(),
                    audio_data.sample_rate
                );

                let mut params = std::collections::BTreeMap::new();
                params.insert(
                    "sample_rate".to_string(),
                    Parameter::Integer(audio_data.sample_rate as i64),
                );
                params.insert("channels".to_string(), Parameter::Integer(1));

                // Convert f32 samples to Arrow ListArray
                let audio_array =
                    dora_node_api::arrow::array::ListArray::from_iter_primitive::<
                        dora_node_api::arrow::datatypes::Float32Type,
                        _,
                        _,
                    >(std::iter::once(Some(
                        audio_data.samples.iter().map(|&s| Some(s)),
                    )));

                if let Err(e) = node.send_output(
                    DataId::from("audio".to_string()),
                    params,
                    audio_array,
                ) {
                    error!("Failed to send audio: {}", e);
                }
            }

            // Receive dora events (if any outputs are expected)
            match events.recv_timeout(std::time::Duration::from_millis(100)) {
                Some(event) => {
                    Self::handle_dora_event(event, &mut node, &event_sender);
                }
                None => {
                    // Timeout or no event, continue
                }
            }
        }

        *state.write() = BridgeState::Disconnected;
        let _ = event_sender.send(BridgeEvent::Disconnected);
        info!("Mic input bridge event loop ended");
    }

    /// Handle a dora event
    fn handle_dora_event(
        event: Event,
        _node: &mut DoraNode,
        event_sender: &Sender<BridgeEvent>,
    ) {
        match event {
            Event::Input { id, data, metadata } => {
                // Mic input typically doesn't receive inputs, but handle if needed
                debug!("Received unexpected input: {}", id);

                // Extract metadata
                let mut event_meta = EventMetadata::default();
                for (key, value) in metadata.parameters.iter() {
                    let string_value = match value {
                        Parameter::String(s) => s.clone(),
                        Parameter::Integer(i) => i.to_string(),
                        Parameter::Float(f) => f.to_string(),
                        Parameter::Bool(b) => b.to_string(),
                        Parameter::ListInt(l) => format!("{:?}", l),
                        Parameter::ListFloat(l) => format!("{:?}", l),
                        Parameter::ListString(l) => format!("{:?}", l),
                    };
                    event_meta.values.insert(key.clone(), string_value);
                }

                let _ = event_sender.try_send(BridgeEvent::DataReceived {
                    input_id: id.to_string(),
                    data: DoraData::Json(serde_json::Value::Null),
                    metadata: event_meta,
                });
            }
            Event::Stop(_) => {
                info!("Received stop event from dora");
            }
            _ => {}
        }
    }
}

impl DoraBridge for MicInputBridge {
    fn node_id(&self) -> &str {
        &self.node_id
    }

    fn state(&self) -> BridgeState {
        *self.state.read()
    }

    fn connect(&mut self) -> BridgeResult<()> {
        if self.is_connected() {
            return Err(BridgeError::AlreadyConnected);
        }

        *self.state.write() = BridgeState::Connecting;

        let (stop_tx, stop_rx) = bounded(1);
        self.stop_sender = Some(stop_tx);

        let node_id = self.node_id.clone();
        let state = Arc::clone(&self.state);
        let event_sender = self.event_sender.clone();
        let audio_receiver = self.audio_receiver.clone();

        let handle = thread::spawn(move || {
            Self::run_event_loop(node_id, state, event_sender, audio_receiver, stop_rx);
        });

        self.worker_handle = Some(handle);

        // Wait for connection result (Connected or Error) with timeout
        let timeout = std::time::Duration::from_secs(5);
        let start = std::time::Instant::now();

        loop {
            match *self.state.read() {
                BridgeState::Connected => return Ok(()),
                BridgeState::Error => {
                    if let Ok(BridgeEvent::Error(msg)) = self.event_receiver.try_recv() {
                        return Err(BridgeError::ConnectionFailed(msg));
                    }
                    return Err(BridgeError::ConnectionFailed("Connection failed".to_string()));
                }
                _ => {}
            }

            if start.elapsed() >= timeout {
                return Err(BridgeError::ConnectionFailed("Connection timeout".to_string()));
            }

            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    fn disconnect(&mut self) -> BridgeResult<()> {
        if let Some(stop_tx) = self.stop_sender.take() {
            let _ = stop_tx.send(());
        }

        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }

        *self.state.write() = BridgeState::Disconnected;
        Ok(())
    }

    fn send(&self, output_id: &str, data: DoraData) -> BridgeResult<()> {
        if !self.is_connected() {
            return Err(BridgeError::NotConnected);
        }

        match (output_id, data) {
            ("audio", DoraData::Audio(audio)) => {
                self.send_audio(audio)?;
            }
            _ => {
                warn!("Unknown output: {}", output_id);
            }
        }

        Ok(())
    }

    fn subscribe(&self) -> Receiver<BridgeEvent> {
        self.event_receiver.clone()
    }

    fn expected_inputs(&self) -> Vec<String> {
        // Mic input doesn't expect any inputs typically
        vec![]
    }

    fn expected_outputs(&self) -> Vec<String> {
        vec!["audio".to_string(), "status".to_string()]
    }
}

impl Drop for MicInputBridge {
    fn drop(&mut self) {
        let _ = self.disconnect();
    }
}
