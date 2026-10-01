use df_types::{ClientBindingId, MemberId, OperationId, RunId, SessionId, TextIdentityError};

#[test]
fn all_identity_kinds_parse_canonical_bytes_in_order() {
    let text = "000102030405060708090a0b0c0d0e0f";
    let expected = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
    assert_eq!(SessionId::from_hex(text).unwrap().as_bytes(), &expected);
    assert_eq!(MemberId::from_hex(text).unwrap().as_bytes(), &expected);
    assert_eq!(
        ClientBindingId::from_hex(text).unwrap().as_bytes(),
        &expected
    );
    assert_eq!(RunId::from_hex(text).unwrap().as_bytes(), &expected);
    assert_eq!(OperationId::from_hex(text).unwrap().as_bytes(), &expected);
}

#[test]
fn each_lowercase_hex_nibble_decodes() {
    let text = "0123456789abcdef0123456789abcdef";
    let expected = [
        0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd,
        0xef,
    ];
    assert_eq!(SessionId::from_hex(text).unwrap().as_bytes(), &expected);
}

#[test]
fn rejects_lengths_before_examining_digits() {
    let short = "0".repeat(31);
    let long_malformed = "g".repeat(33);
    for (text, actual) in [
        ("", 0),
        (" ", 1),
        (short.as_str(), 31),
        (long_malformed.as_str(), 33),
    ] {
        assert_eq!(
            SessionId::from_hex(text),
            Err(TextIdentityError::InvalidLength { actual })
        );
    }
    let oversized_utf8 = format!("{}é", "0".repeat(30));
    assert_eq!(oversized_utf8.len(), 32);
    assert_eq!(
        SessionId::from_hex(&oversized_utf8),
        Err(TextIdentityError::Malformed)
    );
}

#[test]
fn rejects_noncanonical_spelling_and_zero() {
    for text in [
        "0123456789ABCDEF0123456789abcdef",
        "0x0123456789abcdef0123456789abcd",
        "01234567-89abcdef0123456789abcde",
        "0123456789abcdef0123456789abcdeg",
        "0123456789abcdef0123456789abcde ",
    ] {
        assert_eq!(SessionId::from_hex(text), Err(TextIdentityError::Malformed));
    }
    assert_eq!(
        SessionId::from_hex("00000000000000000000000000000000"),
        Err(TextIdentityError::Zero)
    );
}
