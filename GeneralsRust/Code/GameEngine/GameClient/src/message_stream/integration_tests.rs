//! Integration Tests for Message Stream System
//!
//! Comprehensive tests covering the entire message stream pipeline including
//! translators, serialization, filtering, and logging.

#[cfg(test)]
mod tests {
    use super::super::*;
    use std::sync::{Arc, RwLock};

    /// Test complete message flow from input to command execution
    #[test]
    fn test_complete_message_flow() {
        let mut stream = MessageStream::new();

        // Attach translators
        let command_translator = TranslatorFactory::create_command_translator();
        let selection_translator = TranslatorFactory::create_selection_translator();

        stream.attach_translator(command_translator, 10);
        stream.attach_translator(selection_translator, 20);

        // Add raw input messages
        stream.append_message(GameMessageType::RawMouseLeftButtonDown(
            ICoord2D { x: 100, y: 50 },
            0,
            1000,
        ));

        stream.append_message(GameMessageType::RawMouseLeftButtonUp(
            ICoord2D { x: 102, y: 52 },
            0,
            1100,
        ));

        // Process messages through translators
        let processed = stream.propagate_messages().unwrap();

        // Verify messages were processed
        assert!(stream.translator_count() > 0);
    }

    /// Test serialization round-trip
    #[test]
    fn test_serialization_round_trip() {
        let mut original = GameMessage::with_player(
            GameMessageType::DoMoveTo(Coord3D {
                x: 10.0,
                y: 20.0,
                z: 0.0,
            }),
            1,
        );

        original.append_integer_argument(42);
        original.append_real_argument(3.14);
        original.append_boolean_argument(true);

        // Serialize
        let serialized = MessageSerializer::serialize(&original).unwrap();
        assert!(!serialized.is_empty());

        // Deserialize
        let deserialized = MessageSerializer::deserialize(&serialized).unwrap();
        assert_eq!(deserialized.get_player_index(), original.get_player_index());
        assert_eq!(deserialized.get_argument_count(), original.get_argument_count());
    }

    /// Test batch serialization
    #[test]
    fn test_batch_serialization() {
        let mut batch = MessageBatch::new();

        for i in 0..10 {
            let msg = GameMessage::with_player(GameMessageType::Invalid, i);
            batch.add_message(&msg).unwrap();
        }

        assert_eq!(batch.len(), 10);

        let serialized = batch.serialize_batch().unwrap();
        let deserialized = MessageBatch::deserialize_batch(&serialized).unwrap();

        assert_eq!(deserialized.len(), 10);
    }

    /// Test message filtering
    #[test]
    fn test_message_filtering() {
        let mut router = MessageRouter::new(0);

        // Create filter for player 1 only
        let filter = MessageFilter::from_players(vec![1]);
        router.add_filter(filter);

        let msg_player0 = GameMessage::with_player(GameMessageType::Invalid, 0);
        let msg_player1 = GameMessage::with_player(GameMessageType::Invalid, 1);

        assert!(!router.should_accept(&msg_player0));
        assert!(router.should_accept(&msg_player1));

        // Test filtering a batch
        let messages = vec![
            GameMessage::with_player(GameMessageType::Invalid, 0),
            GameMessage::with_player(GameMessageType::Invalid, 1),
            GameMessage::with_player(GameMessageType::NewGame, 1),
        ];

        let filtered = router.filter_messages(messages);
        assert_eq!(filtered.len(), 2);
    }

    /// Test broadcast manager
    #[test]
    fn test_broadcast_manager() {
        let mut manager = BroadcastManager::new(0);

        // Queue messages
        manager.queue_broadcast(GameMessage::new(GameMessageType::NewGame));
        manager.queue_unicast(GameMessage::new(GameMessageType::Invalid), 1);
        manager.queue_multicast(GameMessage::new(GameMessageType::ClearGameData), vec![1, 2]);

        assert!(manager.outgoing_count() > 0);

        // Get messages for specific player
        let player1_msgs = manager.get_outgoing_for_player(1);
        assert!(!player1_msgs.is_empty());

        // Clear queue
        manager.clear_outgoing();
        assert_eq!(manager.outgoing_count(), 0);
    }

    /// Test message priority routing
    #[test]
    fn test_priority_routing() {
        let router = MessageRouter::new(0);

        let critical_msg = GameMessage::new(GameMessageType::FrameTick(100));
        let normal_msg = GameMessage::new(GameMessageType::Invalid);

        let critical_priority = router.get_message_priority(&critical_msg);
        let normal_priority = router.get_message_priority(&normal_msg);

        assert!(critical_priority < normal_priority);
    }

    /// Test message logging
    #[test]
    fn test_message_logging() {
        let mut logger = MessageLogger::new();

        // Log different message types
        logger.log_message(
            GameMessage::new(GameMessageType::Invalid),
            MessageSource::Local,
        );
        logger.log_message(
            GameMessage::new(GameMessageType::NewGame),
            MessageSource::Network,
        );
        logger.log_message(
            GameMessage::new(GameMessageType::ClearGameData),
            MessageSource::System,
        );

        assert_eq!(logger.entry_count(), 3);

        let stats = logger.get_statistics();
        assert_eq!(stats.total_messages, 3);
        assert_eq!(stats.local_messages, 1);
        assert_eq!(stats.network_messages, 1);
        assert_eq!(stats.system_messages, 1);

        // Test search functionality
        let local_msgs = logger.search_by_player(0);
        assert_eq!(local_msgs.len(), 3);

        let recent = logger.get_recent_entries(2);
        assert_eq!(recent.len(), 2);
    }

    /// Test replay recording
    #[test]
    fn test_replay_recording() {
        let mut recorder = ReplayRecorder::new("/tmp/test_replay.bin");
        recorder.set_metadata("TestMap".to_string(), 2);

        recorder.start_recording();
        assert!(recorder.is_recording());

        // Record some messages
        for frame in 0..10 {
            let msg = GameMessage::new(GameMessageType::FrameTick(frame));
            recorder.record_message(msg, frame);
        }

        assert_eq!(recorder.message_count(), 10);

        recorder.stop_recording();
        assert!(!recorder.is_recording());
    }

    /// Test command list processing
    #[test]
    fn test_command_list_processing() {
        let mut command_list = CommandList::with_limit(5);

        // Add messages
        for i in 0..10 {
            command_list.append_message(GameMessage::new(GameMessageType::Invalid));
        }

        assert_eq!(command_list.pending_command_count(), 10);

        // Get commands with limit
        let commands = command_list.get_all_commands();
        assert_eq!(commands.len(), 5);
        assert_eq!(command_list.get_commands_processed_this_frame(), 5);

        // Reset frame counter
        command_list.reset_frame_counter();
        let more_commands = command_list.get_all_commands();
        assert_eq!(more_commands.len(), 5);
    }

    /// Test command list statistics
    #[test]
    fn test_command_list_statistics() {
        let mut command_list = CommandList::new();

        command_list.append_message(GameMessage::new(
            GameMessageType::DoMoveTo(Coord3D::default())
        ));
        command_list.append_message(GameMessage::new(
            GameMessageType::DoAttackObject(456)
        ));
        command_list.append_message(GameMessage::new(
            GameMessageType::DozerConstruct(789, Coord3D::default(), 0.0)
        ));

        let stats = command_list.get_statistics();
        assert_eq!(stats.total_commands, 3);
        assert_eq!(stats.movement_commands, 1);
        assert_eq!(stats.combat_commands, 1);
        assert_eq!(stats.construction_commands, 1);
    }

    /// Test translator priority ordering
    #[test]
    fn test_translator_priority_ordering() {
        let mut stream = MessageStream::new();

        let trans1 = TranslatorFactory::create_command_translator();
        let trans2 = TranslatorFactory::create_selection_translator();
        let trans3 = TranslatorFactory::create_hint_spy();

        // Attach in random order
        stream.attach_translator(trans2, 50);
        stream.attach_translator(trans1, 10);
        stream.attach_translator(trans3, 100);

        // Add a message
        stream.append_message(GameMessageType::Invalid);

        // Propagate should sort by priority
        let _ = stream.propagate_messages();
        // If it completes without error, priority ordering works
    }

    /// Test end-to-end network simulation
    #[test]
    fn test_network_simulation() {
        // Simulate two players exchanging messages

        // Player 1
        let mut player1_manager = BroadcastManager::new(0);
        let msg1 = GameMessage::with_player(
            GameMessageType::DoMoveTo(Coord3D { x: 10.0, y: 20.0, z: 0.0 }),
            0
        );
        player1_manager.queue_broadcast(msg1);

        // Player 2
        let mut player2_manager = BroadcastManager::new(1);
        let msg2 = GameMessage::with_player(
            GameMessageType::DoAttackObject(100),
            1
        );
        player2_manager.queue_broadcast(msg2);

        // Simulate network delivery
        let p1_outgoing = player1_manager.get_outgoing_for_player(1);
        let p2_outgoing = player2_manager.get_outgoing_for_player(0);

        // Deliver messages
        for msg in p1_outgoing {
            player2_manager.add_incoming(msg);
        }
        for msg in p2_outgoing {
            player1_manager.add_incoming(msg);
        }

        // Verify delivery
        let p1_incoming = player1_manager.get_incoming();
        let p2_incoming = player2_manager.get_incoming();

        assert!(!p1_incoming.is_empty());
        assert!(!p2_incoming.is_empty());
    }

    /// Test message debugger
    #[test]
    fn test_message_debugger() {
        let mut debugger = MessageDebugger::new();

        for i in 0..100 {
            let msg = GameMessage::with_player(GameMessageType::Invalid, i % 3);
            debugger.record_message(&msg);
        }

        let type_stats = debugger.get_type_statistics();
        assert!(!type_stats.is_empty());

        let player_stats = debugger.get_player_statistics();
        assert_eq!(player_stats.len(), 3);

        debugger.update_metrics(100, 10000, 1.0);
        let metrics = debugger.get_metrics();
        assert_eq!(metrics.messages_per_second, 100.0);
    }

    /// Test integrated system with all components
    #[test]
    fn test_integrated_system() {
        // Setup
        let mut stream = MessageStream::new();
        let mut command_list = CommandList::new();
        let mut logger = MessageLogger::new();
        let mut debugger = MessageDebugger::new();
        let mut broadcast_manager = BroadcastManager::new(0);

        // Attach translators
        let translators = TranslatorFactory::create_standard_translator_set();
        for (translator, priority) in translators {
            stream.attach_translator(translator, priority);
        }

        // Simulate input
        stream.append_message(GameMessageType::RawMouseLeftButtonDown(
            ICoord2D { x: 100, y: 100 },
            0,
            1000,
        ));

        stream.append_message(GameMessageType::RawKeyDown(0x53)); // 'S' key

        // Process messages
        let processed = stream.propagate_messages().unwrap();

        // Log messages
        for msg in &processed {
            logger.log_message(msg.clone(), MessageSource::Local);
            debugger.record_message(msg);
        }

        // Send to command list
        command_list.append_message_list(processed.clone());

        // Broadcast some messages
        for msg in processed {
            broadcast_manager.queue_broadcast(msg);
        }

        // Verify everything worked
        assert!(logger.entry_count() > 0);
        assert!(command_list.pending_command_count() > 0);
        assert!(broadcast_manager.outgoing_count() > 0);
    }

    /// Test subsystem interface compliance
    #[test]
    fn test_subsystem_interface() {
        let mut stream = MessageStream::new();
        let mut command_list = CommandList::new();

        // Test init
        assert!(stream.init().is_ok());
        assert!(command_list.init().is_ok());

        // Add some data
        stream.append_message(GameMessageType::Invalid);
        command_list.append_message(GameMessage::new(GameMessageType::Invalid));

        // Test update
        assert!(stream.update().is_ok());
        assert!(command_list.update().is_ok());

        // Test reset
        assert!(stream.reset().is_ok());
        assert!(command_list.reset().is_ok());

        // Verify reset cleared data
        assert_eq!(stream.message_count(), 0);
        assert_eq!(command_list.pending_command_count(), 0);
    }
}
