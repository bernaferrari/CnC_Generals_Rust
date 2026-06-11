#![allow(missing_docs)]

//! Message Filtering and Broadcast Systems
//!
//! This module provides filtering, routing, and broadcast capabilities for the
//! message stream system, enabling selective message delivery and network routing.

use super::game_message::*;
use log::{debug, info, warn};
use std::collections::{HashMap, HashSet};

/// Message delivery mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeliveryMode {
    /// Message is delivered only to the local player
    Unicast,
    /// Message is delivered to all players
    Broadcast,
    /// Message is delivered to specific players
    Multicast,
    /// Message is delivered to all players except sender
    BroadcastExceptSender,
}

/// Message priority levels for routing and processing
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MessagePriority {
    Critical = 0,   // Must be processed immediately
    High = 1,       // Process before normal messages
    Normal = 2,     // Standard priority
    Low = 3,        // Can be delayed if needed
    Background = 4, // Process when idle
}

impl Default for MessagePriority {
    fn default() -> Self {
        MessagePriority::Normal
    }
}

/// Message filter criteria
#[derive(Debug, Clone)]
pub struct MessageFilter {
    /// Include only messages from these players (None = all players)
    pub player_filter: Option<HashSet<i32>>,
    /// Include only these message types (None = all types)
    pub type_filter: Option<HashSet<std::mem::Discriminant<GameMessageType>>>,
    /// Minimum priority level to accept
    pub min_priority: MessagePriority,
    /// Custom filter function
    pub custom_filter: Option<fn(&GameMessage) -> bool>,
}

impl MessageFilter {
    /// Create a new filter that accepts all messages
    pub fn accept_all() -> Self {
        Self {
            player_filter: None,
            type_filter: None,
            min_priority: MessagePriority::Background,
            custom_filter: None,
        }
    }

    /// Create a filter for specific players
    pub fn from_players(players: Vec<i32>) -> Self {
        Self {
            player_filter: Some(players.into_iter().collect()),
            type_filter: None,
            min_priority: MessagePriority::Background,
            custom_filter: None,
        }
    }

    /// Create a filter for specific message types
    pub fn from_types(types: Vec<GameMessageType>) -> Self {
        Self {
            player_filter: None,
            type_filter: Some(
                types
                    .into_iter()
                    .map(|t| std::mem::discriminant(&t))
                    .collect(),
            ),
            min_priority: MessagePriority::Background,
            custom_filter: None,
        }
    }

    /// Create a filter for minimum priority
    pub fn from_priority(min_priority: MessagePriority) -> Self {
        Self {
            player_filter: None,
            type_filter: None,
            min_priority,
            custom_filter: None,
        }
    }

    /// Check if a message passes this filter
    pub fn matches(&self, message: &GameMessage, priority: MessagePriority) -> bool {
        // Check priority
        if priority > self.min_priority {
            return false;
        }

        // Check player filter
        if let Some(ref players) = self.player_filter {
            if !players.contains(&message.get_player_index()) {
                return false;
            }
        }

        // Check type filter
        if let Some(ref types) = self.type_filter {
            let msg_discriminant = std::mem::discriminant(message.get_type());
            if !types.contains(&msg_discriminant) {
                return false;
            }
        }

        // Check custom filter
        if let Some(filter_fn) = self.custom_filter {
            if !filter_fn(message) {
                return false;
            }
        }

        true
    }

    /// Add a player to the filter
    pub fn add_player(&mut self, player: i32) {
        if let Some(ref mut players) = self.player_filter {
            players.insert(player);
        } else {
            let mut set = HashSet::new();
            set.insert(player);
            self.player_filter = Some(set);
        }
    }

    /// Remove a player from the filter
    pub fn remove_player(&mut self, player: i32) {
        if let Some(ref mut players) = self.player_filter {
            players.remove(&player);
        }
    }

    /// Add a message type to the filter
    pub fn add_message_type(&mut self, msg_type: GameMessageType) {
        let discriminant = std::mem::discriminant(&msg_type);
        if let Some(ref mut types) = self.type_filter {
            types.insert(discriminant);
        } else {
            let mut set = HashSet::new();
            set.insert(discriminant);
            self.type_filter = Some(set);
        }
    }
}

impl Default for MessageFilter {
    fn default() -> Self {
        Self::accept_all()
    }
}

/// Message with routing metadata
#[derive(Debug, Clone)]
pub struct RoutedMessage {
    pub message: GameMessage,
    pub delivery_mode: DeliveryMode,
    pub priority: MessagePriority,
    pub recipients: Vec<i32>, // Player indices for multicast
    pub sender: i32,
}

impl RoutedMessage {
    /// Create a new routed message with unicast delivery
    pub fn unicast(message: GameMessage, recipient: i32) -> Self {
        Self {
            sender: message.get_player_index(),
            message,
            delivery_mode: DeliveryMode::Unicast,
            priority: MessagePriority::Normal,
            recipients: vec![recipient],
        }
    }

    /// Create a new routed message with broadcast delivery
    pub fn broadcast(message: GameMessage) -> Self {
        Self {
            sender: message.get_player_index(),
            message,
            delivery_mode: DeliveryMode::Broadcast,
            priority: MessagePriority::Normal,
            recipients: Vec::new(),
        }
    }

    /// Create a new routed message with multicast delivery
    pub fn multicast(message: GameMessage, recipients: Vec<i32>) -> Self {
        Self {
            sender: message.get_player_index(),
            message,
            delivery_mode: DeliveryMode::Multicast,
            priority: MessagePriority::Normal,
            recipients,
        }
    }

    /// Set the priority of this message
    pub fn with_priority(mut self, priority: MessagePriority) -> Self {
        self.priority = priority;
        self
    }

    /// Check if this message should be delivered to a specific player
    pub fn should_deliver_to(&self, player_id: i32) -> bool {
        match self.delivery_mode {
            DeliveryMode::Unicast => self.recipients.contains(&player_id),
            DeliveryMode::Broadcast => true,
            DeliveryMode::Multicast => self.recipients.contains(&player_id),
            DeliveryMode::BroadcastExceptSender => player_id != self.sender,
        }
    }
}

/// Message router for network and local delivery
pub struct MessageRouter {
    /// Local player ID
    local_player_id: i32,
    /// Active filters for incoming messages
    filters: Vec<MessageFilter>,
    /// Message priority mapping
    priority_map: HashMap<std::mem::Discriminant<GameMessageType>, MessagePriority>,
}

impl MessageRouter {
    pub fn new(local_player_id: i32) -> Self {
        Self {
            local_player_id,
            filters: Vec::new(),
            priority_map: Self::create_default_priority_map(),
        }
    }

    /// Add a filter to the router
    pub fn add_filter(&mut self, filter: MessageFilter) {
        self.filters.push(filter);
    }

    /// Remove all filters
    pub fn clear_filters(&mut self) {
        self.filters.clear();
    }

    /// Check if a message passes all filters
    pub fn should_accept(&self, message: &GameMessage) -> bool {
        if self.filters.is_empty() {
            return true;
        }

        let priority = self.get_message_priority(message);

        // Message must pass all filters
        self.filters
            .iter()
            .all(|filter| filter.matches(message, priority))
    }

    /// Filter a list of messages
    pub fn filter_messages(&self, messages: Vec<GameMessage>) -> Vec<GameMessage> {
        messages
            .into_iter()
            .filter(|msg| self.should_accept(msg))
            .collect()
    }

    /// Route a message and determine delivery
    pub fn route_message(&self, mut message: GameMessage) -> Vec<RoutedMessage> {
        let mut routed_messages = Vec::new();
        let priority = self.get_message_priority(&message);

        // Determine routing based on message type
        match message.get_type() {
            // Network messages - broadcast to all players
            GameMessageType::CreateSelectedGroup(_, _)
            | GameMessageType::DoAttackSquad(_)
            | GameMessageType::DoMoveTo(_)
            | GameMessageType::DoAttackObject(_)
            | GameMessageType::DozerConstruct(_, _, _)
            | GameMessageType::DozerConstructLine(_, _, _, _)
            | GameMessageType::QueueUnitCreate(_, _) => {
                routed_messages.push(RoutedMessage::broadcast(message).with_priority(priority));
            }

            // Hint messages - local only
            GameMessageType::DoMoveToHint(_)
            | GameMessageType::DoAttackObjectHint(_)
            | GameMessageType::DoInvalidHint => {
                routed_messages.push(
                    RoutedMessage::unicast(message, self.local_player_id)
                        .with_priority(MessagePriority::Low),
                );
            }

            // Meta messages - local only
            GameMessageType::MetaToggleControlBar
            | GameMessageType::MetaOptions
            | GameMessageType::MetaDiplomacy => {
                routed_messages.push(
                    RoutedMessage::unicast(message, self.local_player_id)
                        .with_priority(MessagePriority::Normal),
                );
            }

            // Frame ticks - broadcast
            GameMessageType::FrameTick(_) => {
                routed_messages.push(
                    RoutedMessage::broadcast(message).with_priority(MessagePriority::Critical),
                );
            }

            // Everything else - broadcast
            _ => {
                routed_messages.push(RoutedMessage::broadcast(message).with_priority(priority));
            }
        }

        debug!("Routed message to {} destinations", routed_messages.len());
        routed_messages
    }

    /// Get the priority for a message type
    pub fn get_message_priority(&self, message: &GameMessage) -> MessagePriority {
        let discriminant = std::mem::discriminant(message.get_type());
        self.priority_map
            .get(&discriminant)
            .copied()
            .unwrap_or(MessagePriority::Normal)
    }

    /// Set priority for a message type
    pub fn set_message_type_priority(
        &mut self,
        msg_type: GameMessageType,
        priority: MessagePriority,
    ) {
        let discriminant = std::mem::discriminant(&msg_type);
        self.priority_map.insert(discriminant, priority);
    }

    /// Create default priority mappings
    fn create_default_priority_map(
    ) -> HashMap<std::mem::Discriminant<GameMessageType>, MessagePriority> {
        let mut map = HashMap::new();

        // Critical messages
        map.insert(
            std::mem::discriminant(&GameMessageType::FrameTick(0)),
            MessagePriority::Critical,
        );
        map.insert(
            std::mem::discriminant(&GameMessageType::Timestamp(0)),
            MessagePriority::Critical,
        );

        // High priority messages
        map.insert(
            std::mem::discriminant(&GameMessageType::NewGame),
            MessagePriority::High,
        );
        map.insert(
            std::mem::discriminant(&GameMessageType::ClearGameData),
            MessagePriority::High,
        );

        // Low priority messages (hints)
        map.insert(
            std::mem::discriminant(&GameMessageType::DoMoveToHint(Coord3D::default())),
            MessagePriority::Low,
        );
        map.insert(
            std::mem::discriminant(&GameMessageType::DoInvalidHint),
            MessagePriority::Low,
        );

        map
    }

    /// Get the local player ID
    pub fn get_local_player_id(&self) -> i32 {
        self.local_player_id
    }

    /// Set the local player ID
    pub fn set_local_player_id(&mut self, player_id: i32) {
        info!(
            "Changing local player ID from {} to {}",
            self.local_player_id, player_id
        );
        self.local_player_id = player_id;
    }
}

/// Message broadcast manager
pub struct BroadcastManager {
    /// Router for message delivery
    router: MessageRouter,
    /// Queue of messages waiting to be broadcast
    outgoing_queue: Vec<RoutedMessage>,
    /// Messages received from network
    incoming_queue: Vec<GameMessage>,
}

impl BroadcastManager {
    pub fn new(local_player_id: i32) -> Self {
        Self {
            router: MessageRouter::new(local_player_id),
            outgoing_queue: Vec::new(),
            incoming_queue: Vec::new(),
        }
    }

    /// Queue a message for broadcast
    pub fn queue_broadcast(&mut self, message: GameMessage) {
        let routed = self.router.route_message(message);
        let count = routed.len();
        self.outgoing_queue.extend(routed);
        debug!("Queued {} messages for broadcast", count);
    }

    /// Queue a unicast message
    pub fn queue_unicast(&mut self, message: GameMessage, recipient: i32) {
        let priority = self.router.get_message_priority(&message);
        let routed = RoutedMessage::unicast(message, recipient).with_priority(priority);
        self.outgoing_queue.push(routed);
    }

    /// Queue a multicast message
    pub fn queue_multicast(&mut self, message: GameMessage, recipients: Vec<i32>) {
        let priority = self.router.get_message_priority(&message);
        let routed = RoutedMessage::multicast(message, recipients).with_priority(priority);
        self.outgoing_queue.push(routed);
    }

    /// Get all outgoing messages for a specific player
    pub fn get_outgoing_for_player(&mut self, player_id: i32) -> Vec<GameMessage> {
        let mut messages = Vec::new();

        // Sort by priority
        self.outgoing_queue.sort_by_key(|m| m.priority);

        for routed_msg in &self.outgoing_queue {
            if routed_msg.should_deliver_to(player_id) {
                messages.push(routed_msg.message.clone());
            }
        }

        debug!(
            "Retrieved {} outgoing messages for player {}",
            messages.len(),
            player_id
        );
        messages
    }

    /// Clear outgoing queue
    pub fn clear_outgoing(&mut self) {
        let count = self.outgoing_queue.len();
        self.outgoing_queue.clear();
        debug!("Cleared {} outgoing messages", count);
    }

    /// Add an incoming message from network
    pub fn add_incoming(&mut self, message: GameMessage) {
        if self.router.should_accept(&message) {
            self.incoming_queue.push(message);
        } else {
            debug!("Filtered out incoming message");
        }
    }

    /// Get all incoming messages
    pub fn get_incoming(&mut self) -> Vec<GameMessage> {
        let messages = std::mem::take(&mut self.incoming_queue);
        debug!("Retrieved {} incoming messages", messages.len());
        messages
    }

    /// Get access to the router
    pub fn router_mut(&mut self) -> &mut MessageRouter {
        &mut self.router
    }

    /// Get the number of queued outgoing messages
    pub fn outgoing_count(&self) -> usize {
        self.outgoing_queue.len()
    }

    /// Get the number of queued incoming messages
    pub fn incoming_count(&self) -> usize {
        self.incoming_queue.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_filter() {
        let mut filter = MessageFilter::accept_all();

        let msg1 = GameMessage::with_player(GameMessageType::Invalid, 0);
        let msg2 = GameMessage::with_player(GameMessageType::Invalid, 1);

        assert!(filter.matches(&msg1, MessagePriority::Normal));
        assert!(filter.matches(&msg2, MessagePriority::Normal));

        // Add player filter
        filter.add_player(0);
        assert!(filter.matches(&msg1, MessagePriority::Normal));
        assert!(!filter.matches(&msg2, MessagePriority::Normal));
    }

    #[test]
    fn test_message_priority() {
        let filter = MessageFilter::from_priority(MessagePriority::Normal);

        let msg = GameMessage::new(GameMessageType::Invalid);

        assert!(filter.matches(&msg, MessagePriority::Critical));
        assert!(filter.matches(&msg, MessagePriority::High));
        assert!(filter.matches(&msg, MessagePriority::Normal));
        assert!(!filter.matches(&msg, MessagePriority::Low));
        assert!(!filter.matches(&msg, MessagePriority::Background));
    }

    #[test]
    fn test_routed_message() {
        let msg = GameMessage::new(GameMessageType::Invalid);

        let unicast = RoutedMessage::unicast(msg.clone(), 1);
        assert!(unicast.should_deliver_to(1));
        assert!(!unicast.should_deliver_to(2));

        let broadcast = RoutedMessage::broadcast(msg.clone());
        assert!(broadcast.should_deliver_to(1));
        assert!(broadcast.should_deliver_to(2));

        let multicast = RoutedMessage::multicast(msg, vec![1, 3]);
        assert!(multicast.should_deliver_to(1));
        assert!(!multicast.should_deliver_to(2));
        assert!(multicast.should_deliver_to(3));
    }

    #[test]
    fn test_message_router() {
        let mut router = MessageRouter::new(0);

        // Test default acceptance
        let msg = GameMessage::new(GameMessageType::Invalid);
        assert!(router.should_accept(&msg));

        // Add filter for player 1 only
        let filter = MessageFilter::from_players(vec![1]);
        router.add_filter(filter);

        let msg0 = GameMessage::with_player(GameMessageType::Invalid, 0);
        let msg1 = GameMessage::with_player(GameMessageType::Invalid, 1);

        assert!(!router.should_accept(&msg0));
        assert!(router.should_accept(&msg1));

        // Clear filters
        router.clear_filters();
        assert!(router.should_accept(&msg0));
    }

    #[test]
    fn test_broadcast_manager() {
        let mut manager = BroadcastManager::new(0);

        let msg1 = GameMessage::new(GameMessageType::Invalid);
        let msg2 = GameMessage::new(GameMessageType::NewGame);

        manager.queue_broadcast(msg1);
        manager.queue_unicast(msg2, 1);

        assert!(manager.outgoing_count() > 0);

        let outgoing = manager.get_outgoing_for_player(1);
        assert!(!outgoing.is_empty());

        manager.clear_outgoing();
        assert_eq!(manager.outgoing_count(), 0);
    }

    #[test]
    fn test_incoming_filtering() {
        let mut manager = BroadcastManager::new(0);

        // Add filter for specific player
        let filter = MessageFilter::from_players(vec![1]);
        manager.router_mut().add_filter(filter);

        let msg0 = GameMessage::with_player(GameMessageType::Invalid, 0);
        let msg1 = GameMessage::with_player(GameMessageType::Invalid, 1);

        manager.add_incoming(msg0);
        manager.add_incoming(msg1);

        let incoming = manager.get_incoming();
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].get_player_index(), 1);
    }
}
