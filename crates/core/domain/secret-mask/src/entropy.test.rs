use super::*;
use crate::tests::fake;

#[test]
fn test_shannon_of_one_symbol_is_zero() {
    assert!(shannon("aaaaaaaa").abs() < 1e-9);
}

#[test]
fn test_shannon_of_two_equal_symbols_is_one_bit() {
    assert!((shannon("abab") - 1.0).abs() < 1e-9);
}

#[test]
fn test_shannon_of_four_equal_symbols_is_two_bits() {
    assert!((shannon("abcdabcd") - 2.0).abs() < 1e-9);
}

#[test]
fn test_shannon_of_empty_is_zero() {
    assert!(shannon("").abs() < 1e-9);
}

#[test]
fn test_shannon_counts_characters_not_bytes() {
    // two characters, each two bytes: one bit per character.
    assert!((shannon("\u{e9}\u{e8}\u{e9}\u{e8}") - 1.0).abs() < 1e-9);
}

#[test]
fn test_the_constants_are_the_documented_ones() {
    assert_eq!(MIN_LEN, 6);
    assert_eq!(SHORT_MAX_LEN, 11);
    assert!((SHORT_RATIO - 0.8).abs() < 1e-9);
    assert_eq!(TOKEN_MIN_LEN, 12);
    assert!((TOKEN_RATIO - 0.8).abs() < 1e-9);
    assert_eq!(TOKEN_LOG_CAP, 32);
    assert!((TRANSITION_RATIO - 0.4).abs() < 1e-9);
    assert_eq!(HEX_MIN_LEN, 32);
    assert!((HEX_THRESHOLD - 3.0).abs() < 1e-9);
    assert_eq!(HEX_KEPT_LENGTHS, [40, 64]);
}

#[test]
fn test_a_short_mixed_token_of_distinct_characters_is_random() {
    assert!(looks_random("aB3xY9"));
    assert!(looks_random("Zk2LmX9v"));
}

#[test]
fn test_a_token_under_the_minimum_length_is_not_random() {
    assert!(!looks_random("aB3x9"));
}

#[test]
fn test_a_short_token_needs_all_three_classes() {
    assert!(!looks_random("zk2lmx9v")); // no capital
    assert!(!looks_random("ZK2LMX9V")); // no lower case
    assert!(!looks_random("ZkqLmXgv")); // no digit
}

#[test]
fn test_a_short_repetitive_token_is_not_random() {
    assert!(!looks_random("xxZ3xxxx"));
}

#[test]
fn test_short_words_and_flags_are_not_random() {
    for w in [
        "release",
        "origin",
        "terminal",
        "problems",
        "src/lib.rs",
        "HEAD~1",
        "Cargo.toml",
        "--release",
    ] {
        assert!(!looks_random(w), "{w}");
    }
}

#[test]
fn test_the_ratio_is_against_the_log_of_the_length() {
    // Four distinct characters among eight: 2 bits against log2(8) = 3.
    assert!(!looks_random("xZ3xZ3xZ"));
}

#[test]
fn test_a_long_random_token_is_random() {
    assert!(looks_random(&fake(1, ALNUM, 48)));
}

#[test]
fn test_a_long_block_needs_two_classes_and_mixed_ones() {
    assert!(!looks_random(&fake(2, "abcdefghijklmnopqrstuvwxyz", 48)));
    assert!(!looks_random(&fake(3, "0123456789", 48)));
    assert!(!looks_random(&fake(4, "ABCDEFGHIJKLMNOPQRSTUVWXYZ", 48)));
}

#[test]
fn test_a_long_block_whose_classes_rarely_change_is_not_random() {
    // 3 classes, 20 distinct characters, but few class changes.
    assert!(!looks_random("abcdefghijklmnopqrst0123456789ABCDEF"));
}

#[test]
fn test_a_long_block_of_low_entropy_is_not_random() {
    assert!(!looks_random(&"xZ3".repeat(12)));
}

#[test]
fn test_symbols_are_not_counted_in_the_class_changes() {
    assert!(!looks_random("claude-opus-4-5-20251101"));
}

#[test]
fn test_ordinary_long_blocks_are_not_random() {
    for b in [
        "claude-opus-4-5-20251101",
        "550e8400-e29b-41d4-a716-446655440000",
        "fix-the-parser-for-issue-1234",
        "crates/core/domain/duration-text/src/lib.test.rs",
        "https://github.com/gitleaks/gitleaks/blob/master/config/gitleaks.toml",
        "/home/babbitt/workspace/worktrees/rust-support--secret-mask",
    ] {
        assert!(!looks_random(b), "{b}");
    }
}

#[test]
fn test_a_long_hex_digest_is_random() {
    assert!(looks_random("9e107d9d372bb6826bd81d3542a419d6"));
    assert!(looks_random("9E107D9D372BB6826BD81D3542A419D6"));
}

#[test]
fn test_git_and_sha256_ids_are_not_random() {
    assert!(!looks_random(&fake(5, "0123456789abcdef", 40)));
    assert!(!looks_random(&fake(6, "0123456789ABCDEF", 64)));
}

#[test]
fn test_hex_of_the_lengths_beside_40_and_64_is_random() {
    for n in [32, 39, 41, 48, 63, 65, 96] {
        assert!(looks_random(&fake(7, "0123456789abcdef", n)), "{n}");
    }
}

#[test]
fn test_short_hex_ids_are_not_random() {
    for n in [12, 16, 24, 31] {
        assert!(!looks_random(&fake(8, "0123456789abcdef", n)), "{n}");
    }
}

#[test]
fn test_hex_under_its_minimum_length_is_not_random() {
    assert!(!looks_random("9e107d9d372bb6826bd81d3542a419d"));
    assert!(!looks_random("f48afab4c"));
}

#[test]
fn test_hex_of_low_entropy_is_not_random() {
    assert!(!looks_random(&"a1".repeat(20)));
}

const ALNUM: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

#[test]
fn test_a_short_block_of_hex_digits_is_not_judged_as_a_token() {
    // every character a hex digit, under 32 of them: the hex rule's, never masked.
    assert!(!looks_random("aB3xY9".replace(['x', 'Y'], "c").as_str()));
}
