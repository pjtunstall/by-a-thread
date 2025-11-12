use std::collections::{HashMap, VecDeque};

use crate::net::{ServerNetworkEvent, ServerNetworkHandle};
use shared::net::AppChannel;

#[derive(Default)]
pub struct MockServerNetwork {
    /// **Incoming Event Queue:** Simulates network events.
    /// The `process_events` function drains this by calling `network.get_event()`.
    /// We add to this in tests using `queue_event`.
    /// Example: `ServerNetworkEvent::ClientConnected`, `ServerNetworkEvent::ClientDisconnected`.
    events_to_process: VecDeque<ServerNetworkEvent>,

    /// **Incoming Message Queue (Client -> Server):** Simulates messages from clients.
    /// The `handle_messages` function drains this by calling `network.receive_message(client_id)`.
    /// We add to this in tests using `queue_message` or `queue_raw_message`.
    client_messages: HashMap<u64, VecDeque<Vec<u8>>>,

    /// **Outgoing Message Log (Server -> Specific Client):** A log of serialized binary messages.
    /// This is the "inbox" for each specific client. It's populated by `send_message`
    /// and `broadcast_message_except`.
    /// Tests read this using `get_sent_messages_data(client_id)` to verify what
    /// a specific client received.
    sent_messages: HashMap<u64, Vec<Vec<u8>>>,

    /// **Outgoing Broadcast Log (Server -> All):** A log of serialized binary messages.
    /// This is populated *only* by `broadcast_message`.
    /// Tests read this using `get_broadcast_messages_data()` to verify that a global broadcast
    /// was sent.
    broadcast_messages: Vec<Vec<u8>>,

    /// **Disconnection Log:** A simple list that records which client IDs were
    /// passed to the `network.disconnect()` method.
    /// This lets us test if your code correctly disconnected a client.
    pub disconnected_clients: Vec<u64>,

    /// **Master Client List:** The mock's "source of truth" for who is connected.
    /// This list is used by `clients_id()` and to determine who to send messages
    /// to in `broadcast_message_except`.
    /// We add to this in tests using `add_client`.
    client_ids: Vec<u64>,
}

impl MockServerNetwork {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_client(&mut self, client_id: u64) {
        self.client_ids.push(client_id);
        self.client_messages.entry(client_id).or_default();
        self.sent_messages.entry(client_id).or_default();
    }

    pub fn queue_event(&mut self, event: ServerNetworkEvent) {
        self.events_to_process.push_back(event);
    }

    pub fn queue_message(&mut self, client_id: u64, message: &str) {
        self.client_messages
            .entry(client_id)
            .or_default()
            .push_back(message.as_bytes().to_vec());
    }

    pub fn queue_raw_message(&mut self, client_id: u64, message: Vec<u8>) {
        self.client_messages
            .entry(client_id)
            .or_default()
            .push_back(message);
    }

    pub fn get_sent_messages_data(&mut self, client_id: u64) -> Vec<Vec<u8>> {
        self.sent_messages.entry(client_id).or_default().clone()
    }

    pub fn get_broadcast_messages_data(&self) -> Vec<Vec<u8>> {
        self.broadcast_messages.clone()
    }
}

impl ServerNetworkHandle for MockServerNetwork {
    fn broadcast_message_except(
        &mut self,
        client_id_to_exclude: u64,
        _channel: AppChannel,
        message: Vec<u8>,
    ) {
        for &id in &self.client_ids {
            if id != client_id_to_exclude {
                self.sent_messages
                    .entry(id)
                    .or_default()
                    .push(message.clone());
            }
        }
    }

    fn get_event(&mut self) -> Option<ServerNetworkEvent> {
        self.events_to_process.pop_front()
    }

    fn clients_id(&self) -> Vec<u64> {
        self.client_ids.clone()
    }

    fn receive_message(&mut self, client_id: u64, _channel: AppChannel) -> Option<Vec<u8>> {
        self.client_messages
            .entry(client_id)
            .or_default()
            .pop_front()
    }

    fn send_message(&mut self, client_id: u64, _channel: AppChannel, message: Vec<u8>) {
        self.sent_messages
            .entry(client_id)
            .or_default()
            .push(message);
    }

    fn broadcast_message(&mut self, _channel: AppChannel, message: Vec<u8>) {
        self.broadcast_messages.push(message);
    }

    fn disconnect(&mut self, client_id: u64) {
        self.disconnected_clients.push(client_id);
        self.client_ids.retain(|&id| id != client_id);
    }
}
