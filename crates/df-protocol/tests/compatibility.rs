use df_protocol::{common as wire, contract_fixture as fixture};
use prost::Message;

#[test]
fn generated_presence_distinguishes_absence_from_explicit_zero_and_empty() {
    let absent = fixture::CompatibilityFixture::decode(&[][..]).unwrap();
    let zero = fixture::CompatibilityFixture::decode(&[8, 0][..]).unwrap();
    assert_eq!(absent.protocol_revision, None);
    assert_eq!(zero.protocol_revision, Some(0));
    assert_eq!(zero.encode_to_vec(), [8, 0]);

    let absent = fixture::Capability::decode(&[][..]).unwrap();
    let zero = fixture::Capability::decode(&[8, 0, 16, 0][..]).unwrap();
    assert_eq!((absent.kind, absent.required), (None, None));
    assert_eq!((zero.kind, zero.required), (Some(0), Some(false)));
    assert_eq!(zero.encode_to_vec(), [8, 0, 16, 0]);

    let absent = wire::SessionRevision::decode(&[][..]).unwrap();
    let zero = wire::SessionRevision::decode(&[10, 0, 16, 0][..]).unwrap();
    assert_eq!((absent.epoch, absent.sequence), (None, None));
    assert_eq!(zero.epoch, Some(wire::RecoveryEpoch { value: None }));
    assert_eq!(zero.sequence, Some(0));
    assert_eq!(zero.encode_to_vec(), [10, 0, 16, 0]);
    assert_eq!(
        wire::RecoveryEpoch::decode(&[8, 0][..]).unwrap().value,
        Some(0)
    );

    assert_eq!(wire::SessionId::decode(&[][..]).unwrap().value, None);
    assert_eq!(
        wire::SessionId::decode(&[10, 0][..]).unwrap().value,
        Some(vec![])
    );
    assert_eq!(
        wire::BuildIdentity::decode(&[][..])
            .unwrap()
            .source_revision,
        None
    );
    assert_eq!(
        wire::BuildIdentity::decode(&[10, 0][..])
            .unwrap()
            .source_revision,
        Some(String::new())
    );
}

#[test]
fn unknown_numeric_enum_values_survive_generated_round_trips() {
    // Protobuf negative int32 values occupy ten varint bytes; -1 is not unspecified zero.
    let negative = [8, 255, 255, 255, 255, 255, 255, 255, 255, 255, 1, 16, 0];
    let decoded = fixture::Capability::decode(negative.as_slice()).unwrap();
    assert_eq!(decoded.kind, Some(-1));
    assert_eq!(decoded.required, Some(false));
    assert_eq!(decoded.encode_to_vec(), negative);

    for kind in [i32::MIN, -1, 99, i32::MAX] {
        let value = fixture::Capability {
            kind: Some(kind),
            required: Some(true),
        };
        let decoded = fixture::Capability::decode(value.encode_to_vec().as_slice()).unwrap();
        assert_eq!(decoded, value);
        assert!(fixture::CapabilityKind::try_from(decoded.kind.unwrap()).is_err());
    }
}

#[test]
fn unknown_oneof_tags_leave_known_payloads_usable_without_inventing_a_payload() {
    let future_only = fixture::CompatibilityFixture::decode(&[8, 1, 74, 0][..]).unwrap();
    assert_eq!(future_only.protocol_revision, Some(1));
    assert_eq!(future_only.payload, None);

    // Known revision {epoch {value 1}, sequence 0}; future tag 9 is optional to this reader.
    let known = [8, 1, 26, 6, 10, 2, 8, 1, 16, 0];
    let expected = fixture::CompatibilityFixture::decode(known.as_slice()).unwrap();
    for additive in [
        [known.as_slice(), &[74, 0]].concat(),
        [&[74, 0], known.as_slice()].concat(),
    ] {
        assert_eq!(
            fixture::CompatibilityFixture::decode(additive.as_slice()).unwrap(),
            expected
        );
    }
    // Generated decode preserves present empty messages; domain validation belongs to consumers.
    assert_eq!(
        fixture::CompatibilityFixture::decode(&[8, 1, 26, 0][..])
            .unwrap()
            .payload,
        Some(fixture::compatibility_fixture::Payload::Revision(
            wire::SessionRevision::default()
        ))
    );
}

#[test]
fn malformed_unknown_fields_and_nested_payloads_refuse_generated_decode() {
    let known = [8, 1, 26, 6, 10, 2, 8, 1, 16, 0];
    for malformed in [
        &[0xA0, 0x06, 0x80][..],       // truncated varint
        &[0xA1, 0x06, 1][..],          // truncated fixed64
        &[0xA2, 0x06, 2, 1][..],       // truncated length-delimited value
        &[0xA3, 0x06, 8, 1][..],       // unterminated group
        &[0xA3, 0x06, 0xAC, 0x06][..], // mismatched end-group tag
        &[0xA5, 0x06, 1][..],          // truncated fixed32
        &[0xA6, 0x06][..],             // invalid wire type
        &[0][..],                      // invalid field number
    ] {
        let bytes = [known.as_slice(), malformed].concat();
        assert!(fixture::CompatibilityFixture::decode(bytes.as_slice()).is_err());
    }
    for malformed in [
        &[8, 1, 26, 5, 10][..],                // truncated known payload
        &[8, 1, 26, 5, 10, 3, 8, 1, 0x80][..], // truncated nested epoch key
        &[8, 1, 18, 3, 8, 0x80, 0x80][..],     // truncated nested capability enum
        &[8, 1, 34, 3, 10, 1, 0xFF][..],       // invalid UTF-8 build label
    ] {
        assert!(fixture::CompatibilityFixture::decode(malformed).is_err());
    }
}
