//! Public API regressions using the executed original CRC/RNG fixture.
//! Regeneration and source authority: GeneralsMD/Code/ParityHarness/tests/CRC.md.
//! This integration target runs without Common's opt-in `internal` unit tests.

use game_engine::common::crc::Crc;
use game_engine::common::random_value::{
    RandomState, get_game_logic_random_seed_crc, get_game_logic_random_seed_state,
    get_game_logic_random_value, with_logic_rng_owner,
};
use game_engine::crc::compute_crc_of_value;

const ORIGINAL: &str = include_str!("fixtures/crc_original.txt");
const MIXED_WORDS: [u32; 6] = [0, 1, 0x01020304, 0x80000000, 0xffffffff, 0x89abcdef];

fn hex(value: &str) -> u32 {
    u32::from_str_radix(value, 16).unwrap()
}

fn expected(name: &str) -> u32 {
    ORIGINAL
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>())
        .find(|parts| parts[0] == "crc" && parts[1] == name)
        .map(|parts| hex(parts[2]))
        .unwrap_or_else(|| panic!("missing original fixture {name}"))
}

fn check_bytes(name: &str, bytes: &[u8]) {
    let expected = expected(name);
    assert_eq!(Crc::from_buffer(bytes).get(), expected, "{name}: whole");
    for split in 0..=bytes.len() {
        let mut crc = Crc::new();
        crc.compute_crc(&bytes[..split]);
        crc.compute_crc(&[]);
        crc.compute_crc(&bytes[split..]);
        assert_eq!(crc.get(), expected, "{name}: split {split}");
    }
    for size in [1, 2, 3, 4, 7, 31, 64, 255] {
        let mut crc = Crc::new();
        for chunk in bytes.chunks(size) {
            crc.compute_crc(chunk);
        }
        assert_eq!(crc.get(), expected, "{name}: chunk size {size}");
    }
}

#[test]
fn empty_and_clear_preserve_original_accumulation() {
    check_bytes("empty", &[]);
    check_bytes("max_byte", &[255]);
    check_bytes("two_ones", &[1, 1]);
    let mut crc = Crc::from_buffer(&[255]);
    crc.compute_crc(&[]);
    assert_eq!(crc.get(), expected("max_byte"));
    crc.clear();
    assert_eq!(crc.get(), expected("empty"));
    crc.compute_crc(&[1, 1]);
    assert_eq!(crc.get(), expected("two_ones"));
}

#[test]
fn shifted_high_bit_is_folded_back_in() {
    let mut bytes = vec![0; 25];
    bytes[0] = 128;
    check_bytes("high_before", &bytes);
    bytes.push(0);
    check_bytes("high_after", &bytes);
}

#[test]
fn byte_addition_wraps_like_original_unsigned_int() {
    let mut bytes = vec![1; 31];
    bytes.push(255);
    check_bytes("byte_overflow", &bytes);
}

#[test]
fn carry_addition_wraps_like_original_unsigned_int() {
    check_bytes("carry_overflow", &[1; 33]);
}

#[test]
fn high_bit_and_byte_overflow_keep_original_order() {
    let mut bytes = vec![1; 32];
    bytes.push(255);
    check_bytes("both_overflow", &bytes);
}

#[test]
fn all_byte_values_match_original_at_chunk_boundaries() {
    let bytes: Vec<u8> = (0..768).map(|value| value as u8).collect();
    check_bytes("all_bytes", &bytes);
}

#[test]
fn typed_word_uses_original_x86_little_endian_bytes() {
    let mut crc = Crc::new();
    crc.compute_single(&0x01020304u32);
    assert_eq!(crc.get(), expected("word"));
    check_bytes("word", &[4, 3, 2, 1]);
    assert_eq!(compute_crc_of_value(&0x01020304u32), expected("word"));
}

#[test]
fn typed_arrays_preserve_word_order_without_padding_or_delimiters() {
    let expected_crc = expected("mixed_words");
    let mut singles = Crc::new();
    for word in MIXED_WORDS {
        singles.compute_single(&word);
    }
    let mut multiple = Crc::new();
    multiple.compute_multiple(&MIXED_WORDS);
    assert_eq!(singles.get(), expected_crc);
    assert_eq!(multiple.get(), expected_crc);
    assert_eq!(compute_crc_of_value(&MIXED_WORDS), expected_crc);
    let nested = [
        [MIXED_WORDS[0], MIXED_WORDS[1], MIXED_WORDS[2]],
        [MIXED_WORDS[3], MIXED_WORDS[4], MIXED_WORDS[5]],
    ];
    assert_eq!(compute_crc_of_value(&nested), expected_crc);
    let mut continued = Crc::from_buffer(&[255]);
    continued.compute_single(&[] as &[u32; 0]);
    continued.compute_multiple(&[] as &[u32]);
    assert_eq!(continued.get(), expected("max_byte"));
    assert_eq!(compute_crc_of_value(&[] as &[u32; 0]), 0);
}

#[test]
fn public_rng_crc_tracks_restorable_state_without_consuming_draws() {
    let mut rows = 0;
    for line in ORIGINAL.lines().filter(|line| line.starts_with("rng ")) {
        let parts: Vec<_> = line.split_whitespace().collect();
        let seed = hex(parts[1]);
        let draws: usize = parts[2].parse().unwrap();
        // User-approved deviation: RNG sequences differ from C++. Reuse the
        // original seed/draw checkpoints for current-state CRC and continuation.
        // CRC byte arithmetic above remains compared with the original fixture.
        let mut owner = RandomState::default();
        owner.seed_random(seed);
        with_logic_rng_owner(&mut owner, || {
            for _ in 0..draws {
                get_game_logic_random_value(0, 1000);
            }
        });
        let expected_words = owner.seed_words();
        let expected_crc = compute_crc_of_value(&expected_words);
        let mut untouched = RandomState::default();
        untouched.set_seed_words(expected_words);
        let next = with_logic_rng_owner(&mut owner, || {
            assert_eq!(get_game_logic_random_seed_crc(), expected_crc);
            assert_eq!(get_game_logic_random_seed_crc(), expected_crc);
            assert_eq!(get_game_logic_random_seed_state(), expected_words);
            get_game_logic_random_value(0, 1000)
        });
        let control_next =
            with_logic_rng_owner(&mut untouched, || get_game_logic_random_value(0, 1000));
        assert_eq!(next, control_next);
        assert_eq!(owner.seed_words(), untouched.seed_words());
        rows += 1;
    }
    assert_eq!(rows, 25, "all seed/draw checkpoints must execute");
}
