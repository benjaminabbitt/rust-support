use super::*;

// ---- the published 64-bit vectors (draft-eastlake-fnv, tables 3 and 4)

#[test]
fn test_fnv1a_64_of_empty_input_is_the_offset_basis() {
    assert_eq!(fnv1a_64(b""), 0xcbf2_9ce4_8422_2325);
}

#[test]
fn test_fnv1a_64_matches_published_vector_for_a() {
    assert_eq!(fnv1a_64(b"a"), 0xaf63_dc4c_8601_ec8c);
}

#[test]
fn test_fnv1a_64_matches_published_vector_for_foobar() {
    assert_eq!(fnv1a_64(b"foobar"), 0x8594_4171_f739_67e8);
}

#[test]
fn test_fnv1a_64_matches_published_vectors_with_a_terminating_nul() {
    assert_eq!(fnv1a_64(b"\0"), 0xaf63_bd4c_8601_b7df);
    assert_eq!(fnv1a_64(b"a\0"), 0x089b_e207_b544_f1e4);
    assert_eq!(fnv1a_64(b"foobar\0"), 0x3453_1ca7_168b_8f38);
}

// ---- the 128-bit vectors (computed by an independent model, see the docs)

#[test]
fn test_fnv1a_128_of_empty_input_is_the_offset_basis() {
    assert_eq!(fnv1a_128(b""), 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d);
}

#[test]
fn test_fnv1a_128_matches_the_modelled_vector_for_a() {
    assert_eq!(
        format!("{:032x}", fnv1a_128(b"a")),
        "d228cb696f1a8caf78912b704e4a8964"
    );
}

#[test]
fn test_fnv1a_128_matches_the_modelled_vector_for_foobar() {
    assert_eq!(
        format!("{:032x}", fnv1a_128(b"foobar")),
        "343e1662793c64bf6f0d3597ba446f18"
    );
}

// ---- the algorithm's shape

#[test]
fn test_fnv1a_xors_then_multiplies_so_it_is_not_fnv1() {
    let fnv1_64 = OFFSET_BASIS_64.wrapping_mul(PRIME_64) ^ 0x61;
    assert_ne!(fnv1a_64(b"a"), fnv1_64);
    let fnv1_128 = OFFSET_BASIS_128.wrapping_mul(PRIME_128) ^ 0x61;
    assert_ne!(fnv1a_128(b"a"), fnv1_128);
}

#[test]
fn test_fnv1a_128_low_half_is_not_the_64_bit_hash() {
    assert_ne!(fnv1a_128(b"foobar") as u64, fnv1a_64(b"foobar"));
}

#[test]
fn test_fnv1a_hashes_each_byte_in_order() {
    assert_ne!(fnv1a_64(b"ab"), fnv1a_64(b"ba"));
    assert_ne!(fnv1a_128(b"ab"), fnv1a_128(b"ba"));
    assert_ne!(fnv1a_64(b"\0"), fnv1a_64(b""));
}

// ---- the parameters

#[test]
fn test_parameters_are_the_published_constants() {
    assert_eq!(PRIME_64, (1u64 << 40) + 0x1b3);
    assert_eq!(PRIME_128, (1u128 << 88) + 0x13b);
    assert_eq!(OFFSET_BASIS_64, 14_695_981_039_346_656_037);
    assert_eq!(
        OFFSET_BASIS_128,
        144_066_263_297_769_815_596_495_629_667_062_367_629
    );
}
