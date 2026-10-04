use df_api::{RequestError, RequestField};
use df_protocol::common as wire;
use df_types::{BuildRevision, IdentityError, RevisionError, RevisionLabelError};
use prost::Message;

#[test]
fn generated_identity_presence_and_invalid_values_refuse_without_defaults() {
    assert_eq!(
        df_api::session_id(None),
        Err(RequestError::Missing(RequestField::Session))
    );
    assert_eq!(
        df_api::member_id(None),
        Err(RequestError::Missing(RequestField::Member))
    );
    assert_eq!(
        df_api::run_id(None),
        Err(RequestError::Missing(RequestField::Run))
    );
    assert_eq!(
        df_api::operation_id(None),
        Err(RequestError::Missing(RequestField::Operation))
    );
    assert_eq!(
        df_api::client_binding_id(None),
        Err(RequestError::Missing(RequestField::ClientBinding))
    );
    let absent = wire::SessionId::decode(&[][..]).unwrap();
    assert_eq!(
        df_api::session_id(Some(&absent)),
        Err(RequestError::Missing(RequestField::Session))
    );
    let empty = wire::SessionId::decode(&[10, 0][..]).unwrap();
    assert_eq!(
        df_api::session_id(Some(&empty)),
        Err(RequestError::Identity {
            field: RequestField::Session,
            cause: IdentityError::InvalidLength { actual: 0 },
        })
    );

    for bytes in [vec![1; 15], vec![1; 17], vec![0; 16]] {
        let cause = if bytes.len() == 16 {
            IdentityError::Zero
        } else {
            IdentityError::InvalidLength {
                actual: bytes.len(),
            }
        };
        assert_eq!(
            df_api::session_id(Some(&wire::SessionId {
                value: Some(bytes.clone())
            })),
            Err(RequestError::Identity {
                field: RequestField::Session,
                cause
            })
        );
        assert_eq!(
            df_api::member_id(Some(&wire::MemberId {
                value: Some(bytes.clone())
            })),
            Err(RequestError::Identity {
                field: RequestField::Member,
                cause
            })
        );
        assert_eq!(
            df_api::run_id(Some(&wire::RunId {
                value: Some(bytes.clone())
            })),
            Err(RequestError::Identity {
                field: RequestField::Run,
                cause
            })
        );
        assert_eq!(
            df_api::operation_id(Some(&wire::OperationId {
                value: Some(bytes.clone())
            })),
            Err(RequestError::Identity {
                field: RequestField::Operation,
                cause
            })
        );
        assert_eq!(
            df_api::client_binding_id(Some(&wire::ClientBindingId { value: Some(bytes) })),
            Err(RequestError::Identity {
                field: RequestField::ClientBinding,
                cause
            })
        );
    }
}

#[test]
fn generated_identities_preserve_exact_canonical_bytes_and_kind() {
    let bytes: Vec<u8> = (1..=16).collect();
    let encoded = wire::SessionId {
        value: Some(bytes.clone()),
    }
    .encode_to_vec();
    let decoded = wire::SessionId::decode(encoded.as_slice()).unwrap();
    assert_eq!(
        df_api::session_id(Some(&decoded))
            .unwrap()
            .as_bytes()
            .as_slice(),
        bytes
    );
    assert_eq!(
        df_api::member_id(Some(&wire::MemberId {
            value: Some(bytes.clone())
        }))
        .unwrap()
        .as_bytes()
        .as_slice(),
        bytes
    );
    assert_eq!(
        df_api::client_binding_id(Some(&wire::ClientBindingId {
            value: Some(bytes.clone())
        }))
        .unwrap()
        .as_bytes()
        .as_slice(),
        bytes
    );
    assert_eq!(
        df_api::run_id(Some(&wire::RunId {
            value: Some(bytes.clone())
        }))
        .unwrap()
        .as_bytes()
        .as_slice(),
        bytes
    );
    assert_eq!(
        df_api::operation_id(Some(&wire::OperationId {
            value: Some(bytes.clone())
        }))
        .unwrap()
        .as_bytes()
        .as_slice(),
        bytes
    );
}

#[test]
fn revision_requires_presence_and_preserves_explicit_zero_sequence() {
    assert_eq!(
        df_api::session_revision(None),
        Err(RequestError::Missing(RequestField::Revision))
    );
    for encoded in [&[][..], &[10, 0, 16, 0][..]] {
        let absent = wire::SessionRevision::decode(encoded).unwrap();
        assert_eq!(
            df_api::session_revision(Some(&absent)),
            Err(RequestError::Missing(RequestField::Epoch))
        );
    }
    let absent_sequence = wire::SessionRevision::decode(&[10, 2, 8, 1][..]).unwrap();
    assert_eq!(
        df_api::session_revision(Some(&absent_sequence)),
        Err(RequestError::Missing(RequestField::Sequence))
    );
    let zero_epoch = wire::SessionRevision::decode(&[10, 2, 8, 0, 16, 0][..]).unwrap();
    assert_eq!(
        df_api::session_revision(Some(&zero_epoch)),
        Err(RequestError::Revision(RevisionError::ZeroEpoch))
    );
    let zero_sequence = wire::SessionRevision::decode(&[10, 2, 8, 1, 16, 0][..]).unwrap();
    let revision = df_api::session_revision(Some(&zero_sequence)).unwrap();
    assert_eq!((revision.epoch().get(), revision.sequence()), (1, 0));
    let maximum = wire::SessionRevision {
        epoch: Some(wire::RecoveryEpoch {
            value: Some(u64::MAX),
        }),
        sequence: Some(u64::MAX),
    };
    let maximum = wire::SessionRevision::decode(maximum.encode_to_vec().as_slice()).unwrap();
    let revision = df_api::session_revision(Some(&maximum)).unwrap();
    assert_eq!(
        (revision.epoch().get(), revision.sequence()),
        (u64::MAX, u64::MAX)
    );
}

#[test]
fn build_fields_are_required_bounded_and_errors_do_not_retain_text() {
    assert_eq!(
        df_api::build_identity(None),
        Err(RequestError::Missing(RequestField::Build))
    );
    let absent = wire::BuildIdentity::decode(&[][..]).unwrap();
    let error = df_api::build_identity(Some(&absent)).unwrap_err();
    assert_eq!(
        error,
        RequestError::Build(df_types::BuildIdentityError {
            revision: BuildRevision::Source,
            cause: RevisionLabelError::Missing
        })
    );
    let valid = wire::BuildIdentity {
        source_revision: Some("source-1".into()),
        native_revision: Some("native-1".into()),
        wasm_revision: Some("wasm-1".into()),
        configuration_revision: Some("config-1".into()),
        content_revision: Some("content-1".into()),
    };
    let decoded = wire::BuildIdentity::decode(valid.encode_to_vec().as_slice()).unwrap();
    let mapped = df_api::build_identity(Some(&decoded)).unwrap();
    for (revision, expected) in [
        (BuildRevision::Source, "source-1"),
        (BuildRevision::Native, "native-1"),
        (BuildRevision::Wasm, "wasm-1"),
        (BuildRevision::Configuration, "config-1"),
        (BuildRevision::Content, "content-1"),
    ] {
        assert_eq!(mapped.revision(revision).as_str(), expected);
    }
    for component in [
        BuildRevision::Source,
        BuildRevision::Native,
        BuildRevision::Wasm,
        BuildRevision::Configuration,
        BuildRevision::Content,
    ] {
        let mut missing = valid.clone();
        match component {
            BuildRevision::Source => missing.source_revision = None,
            BuildRevision::Native => missing.native_revision = None,
            BuildRevision::Wasm => missing.wasm_revision = None,
            BuildRevision::Configuration => missing.configuration_revision = None,
            BuildRevision::Content => missing.content_revision = None,
        }
        assert_eq!(
            df_api::build_identity(Some(&missing)),
            Err(RequestError::Build(df_types::BuildIdentityError {
                revision: component,
                cause: RevisionLabelError::Missing
            }))
        );
    }
    for (text, cause) in [
        (String::new(), RevisionLabelError::Empty),
        ("x".repeat(129), RevisionLabelError::TooLong { actual: 129 }),
        (
            "private credential".into(),
            RevisionLabelError::InvalidCharacter { byte_index: 7 },
        ),
    ] {
        let mut invalid = valid.clone();
        invalid.content_revision = Some(text);
        assert_eq!(
            df_api::build_identity(Some(&invalid)),
            Err(RequestError::Build(df_types::BuildIdentityError {
                revision: BuildRevision::Content,
                cause
            }))
        );
    }
}

#[test]
fn malformed_protobuf_is_refused_by_generated_decoder_before_domain_mapping() {
    for encoded in [&[10, 16, 1][..], &[0][..], &[10, 1][..]] {
        assert!(wire::SessionId::decode(encoded).is_err());
    }
    assert!(wire::SessionRevision::decode(&[10, 3, 8, 0x80][..]).is_err());
    assert!(wire::BuildIdentity::decode(&[10, 1, 0xff][..]).is_err());
}
