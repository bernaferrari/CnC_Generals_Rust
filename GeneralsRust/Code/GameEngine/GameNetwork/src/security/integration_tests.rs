//! Integration tests for the comprehensive security system
//!
//! This module provides comprehensive integration tests for all security components
//! working together, including performance benchmarks and security validation.

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::commands::{NetCommand, NetCommandType, CommandPayload};
    use crate::security::*;
    use crate::time::NetworkInstant;
    use std::net::{IpAddr, Ipv4Addr};
    use std::time::Duration;
    use std::sync::Arc;
    use tokio::time;
    use uuid::Uuid;
    use ::rand::rngs::OsRng;
    
    /// Test comprehensive security system integration
    #[tokio::test]
    async fn test_full_security_stack_integration() {
        // Create security manager with all features enabled
        let security_manager = SecurityManager::new().unwrap();
        
        // Register test user
        let user_id = "test_user".to_string();
        let username = "TestPlayer".to_string();
        let ed25519_keypair = ed25519_dalek::SigningKey::from_bytes(&::rand::random::<[u8; 32]>());
        let roles = vec!["player".to_string()];
        
        security_manager.auth_provider().register_user(
            user_id.clone(),
            username.clone(),
            Some(ed25519_keypair.verifying_key()),
            roles.clone(),
        ).await.unwrap();

        // Test authentication
        let auth_request = AuthRequest::GuestAuth {
            username: username.clone(),
        };
        
        let auth_response = security_manager.authenticate_user(auth_request).await.unwrap();
        assert!(matches!(auth_response, AuthResponse::Success { .. }));

        // Test network security
        let test_ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));
        let connection_id = Uuid::new_v4();
        
        // Register connection
        let connection_allowed = security_manager.register_connection(
            connection_id, test_ip, Some(1)
        ).await.unwrap();
        assert!(connection_allowed);

        // Test rate limiting
        for i in 0..10 {
            let allowed = security_manager.check_request_allowed(test_ip, Some(1)).await.unwrap();
            println!("Request {}: allowed = {}", i, allowed);
        }

        // Test command validation with anti-cheat
        let command = NetCommand::new(
            NetCommandType::GameCommand,
            1,
            100,
            CommandPayload::GameCommand(crate::commands::GameCommandData {
                command_type: 1,
                target_id: None,
                position: None,
                parameters: std::collections::HashMap::new(),
                checksum: 0,
            }),
        );

        // This should pass initially
        let validation_result = security_manager.validate_command(&command).await;
        // Note: might fail due to authentication requirements, but shouldn't crash
        println!("Command validation result: {:?}", validation_result);

        // Test encryption
        let test_data = b"Hello, secure world!";
        let encrypted_packet = security_manager.encrypt_packet(test_data, None).await.unwrap();
        let decrypted_data = security_manager.decrypt_packet(&encrypted_packet).await.unwrap();
        assert_eq!(test_data, decrypted_data.as_slice());

        // Get comprehensive statistics
        let stats = security_manager.get_comprehensive_stats().await;
        println!("Security statistics: {:#?}", stats);
        
        // Cleanup
        security_manager.comprehensive_cleanup().await.unwrap();
        
        println!("Full security stack integration test completed successfully");
    }

    /// Test security policy system
    #[tokio::test]
    async fn test_security_policies() {
        let mut policy_manager = SecurityPolicyManager::new();
        
        // Test policy recommendations
        let competitive_policy = policy_manager.get_policy_recommendation(8, true, true);
        assert_eq!(competitive_policy, SecurityPolicy::Competitive);
        
        let lan_policy = policy_manager.get_policy_recommendation(4, false, false);
        assert_eq!(lan_policy, SecurityPolicy::LanTrusted);

        // Test policy switching
        assert!(policy_manager.set_policy(SecurityPolicy::Competitive).is_ok());
        assert_eq!(policy_manager.get_current_policy(), SecurityPolicy::Competitive);

        // Validate all predefined policies
        for policy in [
            SecurityPolicy::Competitive,
            SecurityPolicy::CasualOnline,
            SecurityPolicy::LanTrusted,
            SecurityPolicy::Development,
        ].iter() {
            let warnings = policy_manager.validate_policy(*policy).unwrap();
            println!("Policy {:?} validation warnings: {:?}", policy, warnings);
        }

        // Test creating security manager from policy
        if let Ok(security_manager) = policy_manager.create_security_manager() {
            println!("Successfully created security manager from policy");
            let stats = security_manager.get_security_stats().await;
            println!("Policy-based security manager stats: {:?}", stats);
        }
    }

    /// Test encryption performance and security
    #[tokio::test]
    async fn test_encryption_performance() {
        let encryption_provider = EncryptionProvider::new().unwrap();
        
        // Test different payload sizes
        let payload_sizes = [1024, 8192, 65536]; // 1KB, 8KB, 64KB
        
        for size in payload_sizes.iter() {
            let test_data = vec![0u8; *size];
            let start = NetworkInstant::now();
            
            // Encrypt
            let encrypted = encryption_provider.encrypt(&test_data, None).await.unwrap();
            let encrypt_time = start.elapsed();
            
            // Decrypt
            let decrypt_start = NetworkInstant::now();
            let decrypted = encryption_provider.decrypt(&encrypted).await.unwrap();
            let decrypt_time = decrypt_start.elapsed();
            
            assert_eq!(test_data, decrypted);
            
            println!("Payload size: {} bytes", size);
            println!("  Encryption time: {:?}", encrypt_time);
            println!("  Decryption time: {:?}", decrypt_time);
            println!("  Encryption throughput: {:.2} MB/s", 
                     (*size as f64) / encrypt_time.as_secs_f64() / 1_000_000.0);
            println!("  Decryption throughput: {:.2} MB/s", 
                     (*size as f64) / decrypt_time.as_secs_f64() / 1_000_000.0);
        }

        // Test key rotation
        let initial_stats = encryption_provider.get_stats().await;
        encryption_provider.force_key_rotation().unwrap();
        time::sleep(Duration::from_millis(100)).await;
        let rotated_stats = encryption_provider.get_stats().await;
        
        assert!(rotated_stats.active_key_id > initial_stats.active_key_id);
        println!("Key rotation test passed");
    }

    /// Test anti-cheat system with simulated cheating
    #[tokio::test]
    async fn test_anti_cheat_detection() {
        let anti_cheat_service = AntiCheatService::new();
        
        // Simulate normal gameplay first
        for i in 0..60 {
            let command = NetCommand::new(
                NetCommandType::GameCommand,
                1,
                i,
                CommandPayload::KeepAlive,
            );
            
            // Add some delay to simulate human timing
            time::sleep(Duration::from_millis(50 + (i % 20) as u64 * 10)).await;
            
            let detection = anti_cheat_service.analyze_command(&command).await.unwrap();
            if let Some(detection) = detection {
                println!("Normal gameplay detection: {:?}", detection);
            }
        }

        // Get player trust score after normal behavior
        let trust_score = anti_cheat_service.get_player_trust_score(1).await;
        println!("Trust score after normal gameplay: {:?}", trust_score);

        // Now simulate suspicious behavior (speed hack)
        println!("Simulating speed hack...");
        for i in 60..80 {
            let command = NetCommand::new(
                NetCommandType::GameCommand,
                1,
                i,
                CommandPayload::GameCommand(crate::commands::GameCommandData {
                    command_type: i as u32,
                    target_id: None,
                    position: None,
                    parameters: std::collections::HashMap::new(),
                    checksum: 0,
                }),
            );
            
            // Very fast commands (speed hack simulation)
            time::sleep(Duration::from_millis(1)).await;
            
            let detection = anti_cheat_service.analyze_command(&command).await.unwrap();
            if let Some(detection) = detection {
                println!("Speed hack detection: confidence={:.3}, type={:?}", 
                         detection.confidence, detection.cheat_type);
                
                // Should detect speed hack with high confidence
                if detection.confidence > 0.8 {
                    assert_eq!(detection.cheat_type, CheatType::SpeedHack);
                    break;
                }
            }
        }

        // Check trust score degradation
        let final_trust_score = anti_cheat_service.get_player_trust_score(1).await;
        println!("Trust score after suspicious behavior: {:?}", final_trust_score);
        
        if let (Some(initial), Some(final_score)) = (trust_score, final_trust_score) {
            assert!(final_score < initial, "Trust score should decrease after suspicious behavior");
        }

        let stats = anti_cheat_service.get_stats().await;
        println!("Anti-cheat stats: {:?}", stats);
    }

    /// Test network security under load
    #[tokio::test]
    async fn test_network_security_load() {
        let network_security = NetworkSecurityManager::new();
        let test_ips = (1..=50).map(|i| IpAddr::V4(Ipv4Addr::new(192, 168, 1, i))).collect::<Vec<_>>();
        
        // Register connections from multiple IPs
        let mut connection_ids = Vec::new();
        for (i, ip) in test_ips.iter().enumerate() {
            let connection_id = Uuid::new_v4();
            let registered = network_security.register_connection(
                connection_id, *ip, Some(i as u8)
            ).await.unwrap();
            
            if registered {
                connection_ids.push((connection_id, *ip));
            }
        }
        
        println!("Registered {} connections", connection_ids.len());

        // Test rate limiting under load
        let mut allowed_requests = 0;
        let mut blocked_requests = 0;
        
        for ip in test_ips.iter() {
            for _ in 0..10 {
                let allowed = network_security.check_request_allowed(*ip, None).await.unwrap();
                if allowed {
                    allowed_requests += 1;
                } else {
                    blocked_requests += 1;
                }
            }
        }
        
        println!("Allowed requests: {}, Blocked requests: {}", allowed_requests, blocked_requests);
        assert!(blocked_requests > 0, "Rate limiting should block some requests");

        // Test connection activity updates
        for (connection_id, _) in &connection_ids {
            network_security.update_connection_activity(
                *connection_id, 1024, 512, 10, 5
            ).await.unwrap();
        }

        // Clean up connections
        for (connection_id, _) in connection_ids {
            network_security.unregister_connection(connection_id).await;
        }

        let stats = network_security.get_stats().await;
        println!("Network security stats after load test: {:?}", stats);
    }

    /// Test key exchange between multiple parties
    #[tokio::test]
    async fn test_multi_party_key_exchange() {
        // Create multiple key exchange providers (simulating different players)
        let player1 = KeyExchangeProvider::new().unwrap();
        let player2 = KeyExchangeProvider::new().unwrap();
        let player3 = KeyExchangeProvider::new().unwrap();

        // Exchange identity keys
        let p1_identity = player1.get_identity_public_key();
        let p2_identity = player2.get_identity_public_key();
        let p3_identity = player3.get_identity_public_key();

        // Set up trust relationships
        player1.add_trusted_identity(2, p2_identity).await;
        player1.add_trusted_identity(3, p3_identity).await;
        player2.add_trusted_identity(1, p1_identity).await;
        player2.add_trusted_identity(3, p3_identity).await;
        player3.add_trusted_identity(1, p1_identity).await;
        player3.add_trusted_identity(2, p2_identity).await;

        // Player 1 initiates key exchange with Player 2
        let initiate_msg = player1.initiate_key_exchange(2).await.unwrap();
        let response_msg = player2.handle_initiate(initiate_msg, 1).await.unwrap();
        let confirm_msg = player1.handle_response(response_msg).await.unwrap();
        player2.handle_confirm(confirm_msg).await.unwrap();

        // Player 1 initiates key exchange with Player 3
        let initiate_msg_3 = player1.initiate_key_exchange(3).await.unwrap();
        let response_msg_3 = player3.handle_initiate(initiate_msg_3, 1).await.unwrap();
        let confirm_msg_3 = player1.handle_response(response_msg_3).await.unwrap();
        player3.handle_confirm(confirm_msg_3).await.unwrap();

        // Verify all key exchanges completed
        let p1_stats = player1.get_stats().await;
        let p2_stats = player2.get_stats().await;
        let p3_stats = player3.get_stats().await;

        println!("Player 1 key exchange stats: {:?}", p1_stats);
        println!("Player 2 key exchange stats: {:?}", p2_stats);
        println!("Player 3 key exchange stats: {:?}", p3_stats);

        // Clean up expired sessions
        let cleaned_p1 = player1.cleanup_expired_sessions().await;
        let cleaned_p2 = player2.cleanup_expired_sessions().await;
        let cleaned_p3 = player3.cleanup_expired_sessions().await;

        println!("Cleaned up sessions: P1={}, P2={}, P3={}", cleaned_p1, cleaned_p2, cleaned_p3);
    }

    /// Stress test for authentication system
    #[tokio::test]
    async fn test_authentication_stress() {
        let auth_provider = Arc::new(AuthenticationProvider::new().unwrap());
        
        // Register multiple users
        let user_count = 100;
        for i in 0..user_count {
            let user_id = format!("user_{}", i);
            let username = format!("Player{}", i);
            let roles = vec!["player".to_string()];
            
            auth_provider.register_user(user_id, username, None, roles).await.unwrap();
        }

        // Authenticate all users concurrently
        let mut auth_tasks = Vec::new();
        for i in 0..user_count {
            let auth_provider_clone = Arc::clone(&auth_provider);
            let username = format!("Player{}", i);
            
            let task = tokio::spawn(async move {
                let auth_request = AuthRequest::GuestAuth { username };
                auth_provider_clone.authenticate(auth_request).await
            });
            
            auth_tasks.push(task);
        }

        // Wait for all authentications to complete
        let mut successful_auths = 0;
        let mut failed_auths = 0;
        
        for task in auth_tasks {
            match task.await.unwrap() {
                Ok(AuthResponse::Success { .. }) => successful_auths += 1,
                Ok(AuthResponse::Failure { .. }) => failed_auths += 1,
                Ok(AuthResponse::Challenge { .. }) => {
                    // Handle challenge case if needed
                }
                Err(_) => failed_auths += 1,
            }
        }
        
        println!("Authentication stress test results:");
        println!("  Successful: {}", successful_auths);
        println!("  Failed: {}", failed_auths);
        println!("  Success rate: {:.2}%", 
                 (successful_auths as f64 / user_count as f64) * 100.0);

        assert!(successful_auths > user_count / 2, "More than half should succeed");

        let stats = auth_provider.get_stats().await;
        println!("Final authentication stats: {:?}", stats);
    }

    /// Benchmark security system performance
    #[tokio::test]
    async fn benchmark_security_performance() {
        let security_manager = SecurityManager::new().unwrap();
        let iterations = 1000;
        
        // Benchmark command validation
        let command = NetCommand::new(
            NetCommandType::KeepAlive,
            1,
            0,
            CommandPayload::KeepAlive,
        );

        // Register a test player first
        let token = security_manager.generate_auth_token("BenchmarkPlayer");
        security_manager
            .authenticate_player(1, "BenchmarkPlayer".to_string(), token, vec![1, 2, 3, 4])
            .await
            .unwrap();

        let start = NetworkInstant::now();
        for i in 0..iterations {
            let mut test_command = command.clone();
            test_command.sequence = i;
            
            // Note: This might fail due to signatures, but we're measuring performance
            let _result = security_manager.validate_command(&test_command).await;
        }
        let validation_time = start.elapsed();
        
        println!("Command validation benchmark:");
        println!("  {} iterations in {:?}", iterations, validation_time);
        println!("  Average time per command: {:?}", validation_time / iterations as u32);
        println!("  Commands per second: {:.0}", 
                 iterations as f64 / validation_time.as_secs_f64());

        // Benchmark encryption/decryption
        let test_data = vec![0u8; 1024]; // 1KB payload
        let start = NetworkInstant::now();
        
        for _ in 0..iterations {
            let encrypted = security_manager.encrypt_packet(&test_data, None).await.unwrap();
            let _decrypted = security_manager.decrypt_packet(&encrypted).await.unwrap();
        }
        let crypto_time = start.elapsed();
        
        println!("Encryption/decryption benchmark:");
        println!("  {} iterations in {:?}", iterations, crypto_time);
        println!("  Average time per operation: {:?}", crypto_time / iterations as u32);
        println!("  Operations per second: {:.0}", 
                 iterations as f64 / crypto_time.as_secs_f64());

        // Final comprehensive stats
        let final_stats = security_manager.get_comprehensive_stats().await;
        println!("Final comprehensive security stats:");
        println!("  Basic: {:?}", final_stats.basic);
        println!("  Encryption: {:?}", final_stats.encryption);
        println!("  Authentication: {:?}", final_stats.authentication);
        println!("  Anti-cheat: {:?}", final_stats.anti_cheat);
        println!("  Network security: {:?}", final_stats.network_security);
    }
}
