use bincode_legacy::{DefaultOptions, Options, deserialize, serialize};

#[test]
fn options_default_and_reject_builder_reject_trailing_bytes() {
    let value = 0x1234_5678_u32;
    let encoded = serialize(&value).expect("serialize value");
    let mut with_trailing = encoded.clone();
    with_trailing.push(0xAA);

    assert_eq!(
        DefaultOptions::new().deserialize::<u32>(&encoded).unwrap(),
        value
    );
    assert!(
        DefaultOptions::new()
            .deserialize::<u32>(&with_trailing)
            .is_err(),
        "DefaultOptions follows bincode 1's reject-trailing default"
    );
    assert!(
        DefaultOptions::default()
            .deserialize::<u32>(&with_trailing)
            .is_err()
    );
    assert!(
        DefaultOptions::new()
            .allow_trailing_bytes()
            .reject_trailing_bytes()
            .deserialize::<u32>(&with_trailing)
            .is_err(),
        "the last trailing-byte option wins"
    );
}

#[test]
fn explicit_allow_and_generic_deserialize_keep_legacy_trailing_compatibility() {
    let value = 0x1234_5678_u32;
    let encoded = serialize(&value).expect("serialize value");
    let mut with_trailing = encoded.clone();
    with_trailing.extend_from_slice(&[0xAA, 0xBB]);

    assert_eq!(
        DefaultOptions::new()
            .reject_trailing_bytes()
            .allow_trailing_bytes()
            .deserialize::<u32>(&with_trailing)
            .unwrap(),
        value,
        "allow_trailing_bytes overrides a preceding reject request"
    );
    assert_eq!(
        deserialize::<u32>(&with_trailing).unwrap(),
        value,
        "the root helper keeps its existing allow-trailing behavior"
    );
    assert_eq!(
        DefaultOptions::new().serialize(&value).unwrap(),
        encoded,
        "Options serialization uses the existing fixed-int legacy encoding"
    );
}

#[test]
fn options_and_root_helper_reject_truncated_payloads() {
    let encoded = serialize(&0x1234_5678_u32).expect("serialize value");
    let truncated = &encoded[..encoded.len() - 1];

    assert!(DefaultOptions::new().deserialize::<u32>(truncated).is_err());
    assert!(
        DefaultOptions::new()
            .allow_trailing_bytes()
            .deserialize::<u32>(truncated)
            .is_err()
    );
    assert!(deserialize::<u32>(truncated).is_err());
}
