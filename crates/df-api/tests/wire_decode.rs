use df_api::{
    EncodedCommandFields, RequestError, RequestField, WireDecodeError, WireDecodeLimits,
    decode_build_identity, decode_client_binding_id, decode_member_id, decode_operation_id,
    decode_recovery_epoch, decode_run_id, decode_session_id, decode_session_revision,
    map_encoded_game_command,
};
use df_model::checkpoint::{Basis, CommandInput, EntityId, GameCommand, GameInput};
use df_protocol::common as wire;
use df_types::{
    BuildIdentity, BuildRevision, IdentityError, MemberId, OperationId, RecoveryEpoch,
    RevisionError, RevisionLabelError, RunId, SessionId, SessionRevision,
};
use prost::Message;
use std::mem::size_of;

fn limits() -> WireDecodeLimits {
    WireDecodeLimits {
        maximum_wire_bytes: 4096,
        maximum_owned_bytes: 4096,
    }
}
fn build() -> wire::BuildIdentity {
    wire::BuildIdentity {
        source_revision: Some("source-1".into()),
        native_revision: Some("native-1".into()),
        wasm_revision: Some("wasm-1".into()),
        configuration_revision: Some("config-1".into()),
        content_revision: Some("content-1".into()),
    }
}

#[test]
fn generated_common_bytes_preserve_all_exact_types_presence_and_safe_unknown_fields() {
    let bytes: Vec<u8> = (1..=16).collect();
    let session = wire::SessionId {
        value: Some(bytes.clone()),
    }
    .encode_to_vec();
    let member = wire::MemberId {
        value: Some(bytes.clone()),
    }
    .encode_to_vec();
    let binding = wire::ClientBindingId {
        value: Some(bytes.clone()),
    }
    .encode_to_vec();
    let run = wire::RunId {
        value: Some(bytes.clone()),
    }
    .encode_to_vec();
    let operation = wire::OperationId {
        value: Some(bytes.clone()),
    }
    .encode_to_vec();
    assert_eq!(
        decode_session_id(&session, limits())
            .unwrap()
            .as_bytes()
            .as_slice(),
        bytes
    );
    assert_eq!(
        decode_member_id(&member, limits())
            .unwrap()
            .as_bytes()
            .as_slice(),
        bytes
    );
    assert_eq!(
        decode_client_binding_id(&binding, limits())
            .unwrap()
            .as_bytes()
            .as_slice(),
        bytes
    );
    assert_eq!(
        decode_run_id(&run, limits()).unwrap().as_bytes().as_slice(),
        bytes
    );
    assert_eq!(
        decode_operation_id(&operation, limits())
            .unwrap()
            .as_bytes()
            .as_slice(),
        bytes
    );
    let epoch = wire::RecoveryEpoch { value: Some(2) }.encode_to_vec();
    assert_eq!(decode_recovery_epoch(&epoch, limits()).unwrap().get(), 2);
    let revision = wire::SessionRevision {
        epoch: Some(wire::RecoveryEpoch { value: Some(2) }),
        sequence: Some(0),
    }
    .encode_to_vec();
    let expected = SessionRevision::new(RecoveryEpoch::new(2).unwrap(), 0);
    assert_eq!(
        decode_session_revision(&revision, limits()).unwrap(),
        expected
    );
    let mapped = decode_build_identity(&build().encode_to_vec(), limits()).unwrap();
    for (label, expected) in [
        (BuildRevision::Source, "source-1"),
        (BuildRevision::Native, "native-1"),
        (BuildRevision::Wasm, "wasm-1"),
        (BuildRevision::Configuration, "config-1"),
        (BuildRevision::Content, "content-1"),
    ] {
        assert_eq!(mapped.revision(label).as_str(), expected);
    }
    // Optional future field 100 is ignored by the generated reader but still charged on input.
    let extended = [session.as_slice(), &[0xa0, 0x06, 0x01]].concat();
    assert_eq!(
        decode_session_id(&extended, limits()),
        decode_session_id(&session, limits())
    );
    let mut exact_wire = limits();
    exact_wire.maximum_wire_bytes = session.len();
    assert_eq!(
        decode_session_id(&extended, exact_wire),
        Err(WireDecodeError::WireCapacity)
    );
    let extended = [revision.as_slice(), &[0xa2, 0x06, 0x02, 0x01, 0x02]].concat();
    assert_eq!(
        decode_session_revision(&extended, limits()).unwrap(),
        expected
    );
}

#[test]
fn malformed_and_missing_common_bodies_refuse_with_safe_typed_errors() {
    assert_eq!(
        decode_session_id(&[10, 16, 1], limits()),
        Err(WireDecodeError::Malformed(RequestField::Session))
    );
    assert_eq!(
        decode_session_revision(&[10, 2, 8, 0x80], limits()),
        Err(WireDecodeError::Malformed(RequestField::Revision))
    );
    assert_eq!(
        decode_build_identity(&[10, 1, 0xff], limits()),
        Err(WireDecodeError::Malformed(RequestField::Build))
    );
    assert_eq!(
        decode_recovery_epoch(&[8, 0x80], limits()),
        Err(WireDecodeError::Malformed(RequestField::Epoch))
    );
    assert_eq!(
        decode_session_id(&[], limits()),
        Err(WireDecodeError::Field(RequestError::Missing(
            RequestField::Session
        )))
    );
    assert_eq!(
        decode_session_id(&[10, 0], limits()),
        Err(WireDecodeError::Field(RequestError::Identity {
            field: RequestField::Session,
            cause: IdentityError::InvalidLength { actual: 0 },
        }))
    );
    assert_eq!(
        decode_recovery_epoch(&[8, 0], limits()),
        Err(WireDecodeError::Field(RequestError::Revision(
            RevisionError::ZeroEpoch
        )))
    );
    assert_eq!(
        decode_session_revision(&[10, 2, 8, 2], limits()),
        Err(WireDecodeError::Field(RequestError::Missing(
            RequestField::Sequence
        )))
    );
    let mut private = build();
    private.content_revision = Some("private credential".into());
    let error = decode_build_identity(&private.encode_to_vec(), limits()).unwrap_err();
    assert_eq!(
        error,
        WireDecodeError::Field(RequestError::Build(df_types::BuildIdentityError {
            revision: BuildRevision::Content,
            cause: RevisionLabelError::InvalidCharacter { byte_index: 7 },
        }))
    );
    assert!(!format!("{error:?}").contains("private credential"));
}

#[test]
fn wire_capacity_precedes_codec_and_owned_capacity_charges_actual_allocations() {
    let mut bounded = limits();
    bounded.maximum_wire_bytes = 1;
    assert_eq!(
        decode_session_id(&[0, 0], bounded),
        Err(WireDecodeError::WireCapacity)
    );
    bounded.maximum_wire_bytes = 0;
    assert_eq!(
        decode_session_id(&[], bounded),
        Err(WireDecodeError::InvalidLimits)
    );
    bounded = limits();
    bounded.maximum_owned_bytes = 1;
    assert_eq!(
        decode_session_id(&[0], bounded),
        Err(WireDecodeError::OwnedCapacity)
    );
    let message = wire::SessionId {
        value: Some(vec![1; 16]),
    };
    let encoded = message.encode_to_vec();
    let generated = wire::SessionId::decode(encoded.as_slice()).unwrap();
    let retained =
        size_of::<wire::SessionId>() + size_of::<SessionId>() + generated.value.unwrap().capacity();
    bounded = limits();
    bounded.maximum_owned_bytes = retained;
    assert_eq!(
        decode_session_id(&encoded, bounded),
        SessionId::from_bytes(&[1; 16]).map_err(|cause| WireDecodeError::Field(
            RequestError::Identity {
                field: RequestField::Session,
                cause,
            }
        ))
    );
    bounded.maximum_owned_bytes = retained - 1;
    assert_eq!(
        decode_session_id(&encoded, bounded),
        Err(WireDecodeError::OwnedCapacity)
    );
    let encoded = build().encode_to_vec();
    let generated = wire::BuildIdentity::decode(encoded.as_slice()).unwrap();
    let strings = [
        generated.source_revision.as_ref(),
        generated.native_revision.as_ref(),
        generated.wasm_revision.as_ref(),
        generated.configuration_revision.as_ref(),
        generated.content_revision.as_ref(),
    ];
    let expected = BuildIdentity::new(
        Some("source-1"),
        Some("native-1"),
        Some("wasm-1"),
        Some("config-1"),
        Some("content-1"),
    )
    .unwrap();
    let wire_heap: usize = strings
        .into_iter()
        .map(|label| label.unwrap().capacity())
        .sum();
    let domain_heap: usize = [
        BuildRevision::Source,
        BuildRevision::Native,
        BuildRevision::Wasm,
        BuildRevision::Configuration,
        BuildRevision::Content,
    ]
    .into_iter()
    .map(|label| expected.revision(label).retained_heap_bytes())
    .sum();
    bounded = limits();
    bounded.maximum_owned_bytes =
        size_of::<wire::BuildIdentity>() + size_of::<BuildIdentity>() + wire_heap + domain_heap;
    assert_eq!(decode_build_identity(&encoded, bounded).unwrap(), expected);
    bounded.maximum_owned_bytes -= 1;
    assert_eq!(
        decode_build_identity(&encoded, bounded),
        Err(WireDecodeError::OwnedCapacity)
    );
}

#[test]
fn bounded_common_bytes_map_to_owned_closed_command_without_a_global_revision_lock() {
    let session = wire::SessionId {
        value: Some(vec![1; 16]),
    }
    .encode_to_vec();
    let run = wire::RunId {
        value: Some(vec![2; 16]),
    }
    .encode_to_vec();
    let operation = wire::OperationId {
        value: Some(vec![20; 16]),
    }
    .encode_to_vec();
    let revision = wire::SessionRevision {
        epoch: Some(wire::RecoveryEpoch { value: Some(2) }),
        sequence: Some(0),
    }
    .encode_to_vec();
    let trusted_member = MemberId::from_bytes(&[3; 16]).unwrap();
    let command = || GameCommand::Speak {
        speaker: EntityId::from_bytes(&[4; 16]).unwrap(),
        text: "hello".into(),
        conversation: None,
    };
    let fields = || EncodedCommandFields {
        session: Some(&session),
        run: Some(&run),
        operation: Some(&operation),
        observed_revision: Some(&revision),
    };
    let input =
        map_encoded_game_command(fields(), trusted_member, command(), limits(), 8192).unwrap();
    assert_eq!(
        input,
        GameInput::Game(CommandInput {
            basis: Basis {
                session: SessionId::from_bytes(&[1; 16]).unwrap(),
                run: RunId::from_bytes(&[2; 16]).unwrap(),
                revision: SessionRevision::new(RecoveryEpoch::new(2).unwrap(), 0)
            },
            observed_revision: SessionRevision::new(RecoveryEpoch::new(2).unwrap(), 0),
            operation: OperationId::from_bytes(&[20; 16]).unwrap(),
            member: trusted_member,
            command: command(),
        })
    );
    let mut missing = fields();
    missing.operation = None;
    assert_eq!(
        map_encoded_game_command(missing, trusted_member, command(), limits(), 8192),
        Err(WireDecodeError::Field(RequestError::Missing(
            RequestField::Operation
        )))
    );
    let mut short = limits();
    short.maximum_wire_bytes = session.len() + run.len() + operation.len();
    assert_eq!(
        map_encoded_game_command(fields(), trusted_member, command(), short, 8192),
        Err(WireDecodeError::WireCapacity)
    );
    assert_eq!(
        map_encoded_game_command(fields(), trusted_member, command(), limits(), 1),
        Err(WireDecodeError::OwnedCapacity)
    );
    let mut reserved = String::with_capacity(8192);
    reserved.push_str("hello");
    let large = GameCommand::Speak {
        speaker: EntityId::from_bytes(&[4; 16]).unwrap(),
        text: reserved,
        conversation: None,
    };
    assert_eq!(
        map_encoded_game_command(fields(), trusted_member, large, limits(), 8192),
        Err(WireDecodeError::OwnedCapacity)
    );
}
