//! Example usage of C++-compatible serialization
//!
//! This example demonstrates how to use the C++ compatibility layer
//! for network communication between Rust and C++ components.

use game_network::commands::cpp_compat_serialization::{
    deserialize_command_cpp_compat, serialize_command_cpp_compat, NetCommandRef,
};
use game_network::commands::{ChatData, CommandPayload, NetCommand, NetCommandType};
use std::collections::HashMap;

fn main() {
    println!("=== C++ Compatibility Serialization Examples ===\n");

    // Example 1: Serialize a simple command for C++
    example_serialize_keep_alive();

    // Example 2: Deserialize a C++ packet
    example_deserialize_cpp_packet();

    // Example 3: Round-trip conversion
    example_round_trip();

    // Example 4: Working with game commands
    example_game_command();

    // Example 5: Handling chat messages
    example_chat_message();
}

/// Example 1: Serialize a KeepAlive command for sending to C++
fn example_serialize_keep_alive() {
    println!("Example 1: Serialize KeepAlive for C++\n");

    // Create a Rust NetCommand
    let rust_command = NetCommand::keep_alive(1);

    // Convert to C++ compatible format
    let cpp_ref = NetCommandRef::from_net_command(&rust_command);

    // Serialize to bytes that C++ can understand
    let bytes = serialize_command_cpp_compat(&cpp_ref);

    println!("Serialized KeepAlive command:");
    println!("  Size: {} bytes", bytes.len());
    println!("  Hex: {}", hex_dump(&bytes));
    println!("  Format breakdown:");
    println!("    'T' (0x{:02X}) + type (0x{:02X})", bytes[0], bytes[1]);
    println!("    'F' (0x{:02X}) + frame (4 bytes)", bytes[2]);
    println!("    'R' (0x{:02X}) + relay (0x{:02X})", bytes[7], bytes[8]);
    println!(
        "    'P' (0x{:02X}) + player (0x{:02X})",
        bytes[9], bytes[10]
    );
    println!("    'C' (0x{:02X}) + id (2 bytes)", bytes[11]);
    println!("    'D' (0x{:02X}) + data\n", bytes[14]);
}

/// Example 2: Deserialize a packet received from C++
fn example_deserialize_cpp_packet() {
    println!("Example 2: Deserialize C++ Packet\n");

    // Simulate bytes received from C++ client
    // This is a KeepAlive command: player 2, id 100, frame 500
    let cpp_bytes = vec![
        b'T', 9, // Type: KeepAlive
        b'F', 0xF4, 0x01, 0x00, 0x00, // Frame: 500
        b'R', 0, // Relay: 0
        b'P', 2, // Player: 2
        b'C', 100, 0,    // ID: 100
        b'D', // Data (none for KeepAlive)
    ];

    println!("Received from C++: {} bytes", cpp_bytes.len());
    println!("Hex: {}", hex_dump(&cpp_bytes));

    // Deserialize from C++ format
    match deserialize_command_cpp_compat(&cpp_bytes) {
        Ok(cmd_ref) => {
            println!("\nParsed command:");
            println!("  Type: {:?}", cmd_ref.command_type);
            println!("  Player ID: {}", cmd_ref.player_id);
            println!("  Command ID: {}", cmd_ref.id);
            println!("  Frame: {}", cmd_ref.execution_frame);
            println!("  Relay: {}\n", cmd_ref.relay);
        }
        Err(e) => {
            eprintln!("Failed to deserialize: {:?}\n", e);
        }
    }
}

/// Example 3: Round-trip conversion (Rust → C++ → Rust)
fn example_round_trip() {
    println!("Example 3: Round-Trip Conversion\n");

    // Create a Rust command
    let original = NetCommand::chat(0, "Hello from Rust!".to_string(), 0xFF);

    println!("Original Rust command:");
    println!("  Type: {:?}", original.command_type);
    if let CommandPayload::Chat(ref chat) = original.payload {
        println!("  Message: {}", chat.message);
        println!("  Target: 0x{:02X}", chat.target_mask);
    }

    // Convert to C++ format
    let cpp_ref = NetCommandRef::from_net_command(&original);

    // Serialize
    let bytes = serialize_command_cpp_compat(&cpp_ref);
    println!("\nSerialized: {} bytes", bytes.len());

    // Deserialize (simulating C++ → Rust)
    let deserialized_ref = deserialize_command_cpp_compat(&bytes).unwrap();

    // Convert back to Rust
    let restored = deserialized_ref.to_net_command();

    println!("\nRestored Rust command:");
    println!("  Type: {:?}", restored.command_type);
    if let CommandPayload::Chat(ref chat) = restored.payload {
        println!("  Message: {}", chat.message);
        println!("  Target: 0x{:02X}", chat.target_mask);
    }

    println!("  Round-trip successful!\n");
}

/// Example 4: Working with game commands
fn example_game_command() {
    println!("Example 4: Game Command with Parameters\n");

    // Create a game command with parameters
    let mut params = HashMap::new();
    params.insert(
        "target_id".to_string(),
        game_network::commands::CommandParameter::ObjectId(12345),
    );
    params.insert(
        "speed".to_string(),
        game_network::commands::CommandParameter::Float(2.5),
    );

    let game_data = game_network::commands::GameCommandData {
        command_type: 1, // Move command
        target_id: Some(12345),
        position: Some((100.0, 200.0, 0.0)),
        parameters: params,
        checksum: 0x12345678,
    };

    let command = NetCommand::game_command(0, 1000, game_data);

    // Convert and serialize
    let cpp_ref = NetCommandRef::from_net_command(&command);
    let bytes = serialize_command_cpp_compat(&cpp_ref);

    println!("GameCommand serialized:");
    println!("  Size: {} bytes", bytes.len());
    println!(
        "  First 32 bytes: {}",
        hex_dump(&bytes[..32.min(bytes.len())])
    );

    // Deserialize back
    let deserialized = deserialize_command_cpp_compat(&bytes).unwrap();

    if let CommandPayload::GameCommand(ref data) = deserialized.payload {
        println!("\nDeserialized GameCommand:");
        println!("  Command type: {}", data.command_type);
        println!("  Target ID: {:?}", data.target_id);
        println!("  Position: {:?}", data.position);
        println!("  Parameters: {} params", data.parameters.len());
        println!("  Checksum: 0x{:08X}\n", data.checksum);
    }
}

/// Example 5: Handling chat messages with Unicode
fn example_chat_message() {
    println!("Example 5: Chat Messages with Unicode\n");

    // Create a chat message with Unicode characters
    let chat_cmd = NetCommandRef {
        command_type: NetCommandType::Chat,
        relay: 0,
        player_id: 1,
        id: 42,
        execution_frame: 0,
        payload: CommandPayload::Chat(ChatData {
            message: "Hello 世界! 🎮".to_string(),
            target_mask: 0xFF, // All players
        }),
    };

    println!("Original message: Hello 世界! 🎮");

    // Serialize
    let bytes = serialize_command_cpp_compat(&chat_cmd);
    println!("Serialized: {} bytes", bytes.len());

    // Deserialize
    let deserialized = deserialize_command_cpp_compat(&bytes).unwrap();

    if let CommandPayload::Chat(ref chat) = deserialized.payload {
        println!("Deserialized message: {}", chat.message);
        println!("Target mask: 0x{:02X}", chat.target_mask);
        println!(
            "UTF-8 encoding preserved: {}\n",
            chat.message == "Hello 世界! 🎮"
        );
    }
}

/// Helper function to format bytes as hex
fn hex_dump(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_examples_run() {
        // Just verify the examples compile and run without panicking
        example_serialize_keep_alive();
        example_deserialize_cpp_packet();
        example_round_trip();
        example_game_command();
        example_chat_message();
    }
}
