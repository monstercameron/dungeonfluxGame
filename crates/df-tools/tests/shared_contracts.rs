use df_protocol::{common as wire, contract_fixture as fixture};
use df_types::{
    BuildIdentity, BuildIdentityError, BuildRevision, ClientBindingId, IdentityError, MemberId,
    OperationId, RecoveryEpoch, RevisionError, RevisionLabel, RevisionLabelError, RunId, SessionId,
    SessionRevision,
};
use prost::Message;

#[derive(Debug, PartialEq, Eq)]
enum ContractError {
    Missing(&'static str),
    Identity(IdentityError),
    Revision(RevisionError),
    Build(BuildIdentityError),
    UnsupportedProtocol(u32),
    UnknownRequiredCapability(i32),
    UnspecifiedCapability,
    MissingPayload,
}

#[derive(Debug, PartialEq, Eq)]
struct Identities {
    session: SessionId,
    member: MemberId,
    client_binding: ClientBindingId,
    run: RunId,
    operation: OperationId,
}

impl TryFrom<fixture::Identities> for Identities {
    type Error = ContractError;

    fn try_from(value: fixture::Identities) -> Result<Self, Self::Error> {
        fn bytes(value: Option<Vec<u8>>, field: &'static str) -> Result<Vec<u8>, ContractError> {
            value.ok_or(ContractError::Missing(field))
        }
        Ok(Self {
            session: SessionId::from_bytes(&bytes(
                value.session.and_then(|id| id.value),
                "session",
            )?)
            .map_err(ContractError::Identity)?,
            member: MemberId::from_bytes(&bytes(value.member.and_then(|id| id.value), "member")?)
                .map_err(ContractError::Identity)?,
            client_binding: ClientBindingId::from_bytes(&bytes(
                value.client_binding.and_then(|id| id.value),
                "client_binding",
            )?)
            .map_err(ContractError::Identity)?,
            run: RunId::from_bytes(&bytes(value.run.and_then(|id| id.value), "run")?)
                .map_err(ContractError::Identity)?,
            operation: OperationId::from_bytes(&bytes(
                value.operation.and_then(|id| id.value),
                "operation",
            )?)
            .map_err(ContractError::Identity)?,
        })
    }
}

impl From<&Identities> for fixture::Identities {
    fn from(value: &Identities) -> Self {
        Self {
            session: Some(wire::SessionId {
                value: Some(value.session.as_bytes().to_vec()),
            }),
            member: Some(wire::MemberId {
                value: Some(value.member.as_bytes().to_vec()),
            }),
            client_binding: Some(wire::ClientBindingId {
                value: Some(value.client_binding.as_bytes().to_vec()),
            }),
            run: Some(wire::RunId {
                value: Some(value.run.as_bytes().to_vec()),
            }),
            operation: Some(wire::OperationId {
                value: Some(value.operation.as_bytes().to_vec()),
            }),
        }
    }
}

fn read_revision(value: wire::SessionRevision) -> Result<SessionRevision, ContractError> {
    let epoch = value
        .epoch
        .and_then(|epoch| epoch.value)
        .ok_or(ContractError::Missing("epoch"))?;
    let epoch = RecoveryEpoch::new(epoch).map_err(ContractError::Revision)?;
    let sequence = value.sequence.ok_or(ContractError::Missing("sequence"))?;
    Ok(SessionRevision::new(epoch, sequence))
}

fn write_revision(value: SessionRevision) -> wire::SessionRevision {
    wire::SessionRevision {
        epoch: Some(wire::RecoveryEpoch {
            value: Some(value.epoch().get()),
        }),
        sequence: Some(value.sequence()),
    }
}

fn read_build(value: wire::BuildIdentity) -> Result<BuildIdentity, ContractError> {
    BuildIdentity::new(
        value.source_revision.as_deref(),
        value.native_revision.as_deref(),
        value.wasm_revision.as_deref(),
        value.configuration_revision.as_deref(),
        value.content_revision.as_deref(),
    )
    .map_err(ContractError::Build)
}

fn write_build(value: &BuildIdentity) -> wire::BuildIdentity {
    wire::BuildIdentity {
        source_revision: Some(value.revision(BuildRevision::Source).as_str().to_owned()),
        native_revision: Some(value.revision(BuildRevision::Native).as_str().to_owned()),
        wasm_revision: Some(value.revision(BuildRevision::Wasm).as_str().to_owned()),
        configuration_revision: Some(
            value
                .revision(BuildRevision::Configuration)
                .as_str()
                .to_owned(),
        ),
        content_revision: Some(value.revision(BuildRevision::Content).as_str().to_owned()),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Payload {
    Revision(SessionRevision),
    Build(BuildIdentity),
}

fn consume(value: fixture::CompatibilityFixture) -> Result<Payload, ContractError> {
    let protocol = value
        .protocol_revision
        .ok_or(ContractError::Missing("protocol_revision"))?;
    if protocol != 1 {
        return Err(ContractError::UnsupportedProtocol(protocol));
    }
    for capability in value.capabilities {
        let kind = capability
            .kind
            .ok_or(ContractError::Missing("capability.kind"))?;
        let required = capability
            .required
            .ok_or(ContractError::Missing("capability.required"))?;
        match fixture::CapabilityKind::try_from(kind) {
            Ok(fixture::CapabilityKind::Unspecified) => {
                return Err(ContractError::UnspecifiedCapability);
            }
            Ok(fixture::CapabilityKind::CompositeRecovery) => {}
            Err(_) if required => return Err(ContractError::UnknownRequiredCapability(kind)),
            Err(_) => {}
        }
    }
    match value.payload.ok_or(ContractError::MissingPayload)? {
        fixture::compatibility_fixture::Payload::Revision(value) => {
            read_revision(value).map(Payload::Revision)
        }
        fixture::compatibility_fixture::Payload::Build(value) => {
            read_build(value).map(Payload::Build)
        }
    }
}

fn revision_fixture(epoch: u64, sequence: u64) -> fixture::CompatibilityFixture {
    fixture::CompatibilityFixture {
        protocol_revision: Some(1),
        capabilities: vec![fixture::Capability {
            kind: Some(1),
            required: Some(true),
        }],
        payload: Some(fixture::compatibility_fixture::Payload::Revision(
            wire::SessionRevision {
                epoch: Some(wire::RecoveryEpoch { value: Some(epoch) }),
                sequence: Some(sequence),
            },
        )),
    }
}

#[test]
fn distinct_supplied_ids_round_trip_canonical_bytes() {
    let value = Identities {
        session: SessionId::from_hex("01010101010101010101010101010101").unwrap(),
        member: MemberId::from_hex("02020202020202020202020202020202").unwrap(),
        client_binding: ClientBindingId::from_hex("03030303030303030303030303030303").unwrap(),
        run: RunId::from_hex("04040404040404040404040404040404").unwrap(),
        operation: OperationId::from_hex("05050505050505050505050505050505").unwrap(),
    };
    let encoded = fixture::Identities::from(&value).encode_to_vec();
    assert_eq!(
        Identities::try_from(fixture::Identities::decode(encoded.as_slice()).unwrap()),
        Ok(value)
    );
    // Different identity types cannot be assigned to each other; no conversion is defined.
    assert_ne!(
        std::any::TypeId::of::<SessionId>(),
        std::any::TypeId::of::<MemberId>()
    );
}

#[test]
fn every_identity_refuses_missing_empty_short_long_and_zero_values() {
    for bytes in [vec![], vec![1; 15], vec![1; 17], vec![0; 16]] {
        assert!(SessionId::from_bytes(&bytes).is_err());
        assert!(MemberId::from_bytes(&bytes).is_err());
        assert!(ClientBindingId::from_bytes(&bytes).is_err());
        assert!(RunId::from_bytes(&bytes).is_err());
        assert!(OperationId::from_bytes(&bytes).is_err());
    }
    let valid = fixture::Identities {
        session: Some(wire::SessionId {
            value: Some(vec![1; 16]),
        }),
        member: Some(wire::MemberId {
            value: Some(vec![2; 16]),
        }),
        client_binding: Some(wire::ClientBindingId {
            value: Some(vec![3; 16]),
        }),
        run: Some(wire::RunId {
            value: Some(vec![4; 16]),
        }),
        operation: Some(wire::OperationId {
            value: Some(vec![5; 16]),
        }),
    };
    for (index, field) in ["session", "member", "client_binding", "run", "operation"]
        .into_iter()
        .enumerate()
    {
        for bytes in [
            None,
            Some(vec![]),
            Some(vec![1; 15]),
            Some(vec![1; 17]),
            Some(vec![0; 16]),
        ] {
            let mut value = valid.clone();
            match index {
                0 => value.session.as_mut().unwrap().value = bytes.clone(),
                1 => value.member.as_mut().unwrap().value = bytes.clone(),
                2 => value.client_binding.as_mut().unwrap().value = bytes.clone(),
                3 => value.run.as_mut().unwrap().value = bytes.clone(),
                4 => value.operation.as_mut().unwrap().value = bytes.clone(),
                _ => unreachable!(),
            }
            let decoded = fixture::Identities::decode(value.encode_to_vec().as_slice()).unwrap();
            let expected = match bytes {
                None => ContractError::Missing(field),
                Some(value) if value.len() != 16 => {
                    ContractError::Identity(IdentityError::InvalidLength {
                        actual: value.len(),
                    })
                }
                Some(_) => ContractError::Identity(IdentityError::Zero),
            };
            assert_eq!(Identities::try_from(decoded), Err(expected));
        }
    }
    let mut missing = valid.clone();
    missing.session = None;
    assert_eq!(
        Identities::try_from(missing),
        Err(ContractError::Missing("session"))
    );
    let mut missing = valid.clone();
    missing.member.as_mut().unwrap().value = None;
    assert_eq!(
        Identities::try_from(missing),
        Err(ContractError::Missing("member"))
    );
    let mut malformed = valid;
    malformed.operation.as_mut().unwrap().value = Some(vec![0; 16]);
    assert_eq!(
        Identities::try_from(malformed),
        Err(ContractError::Identity(IdentityError::Zero))
    );
}

#[test]
fn recovery_round_trip_orders_newer_epoch_before_sequence_and_checks_overflow() {
    let older = SessionRevision::new(RecoveryEpoch::new(7).unwrap(), u64::MAX);
    let newer = SessionRevision::new(RecoveryEpoch::new(8).unwrap(), 0);
    let decode = |value| {
        read_revision(
            wire::SessionRevision::decode(write_revision(value).encode_to_vec().as_slice())
                .unwrap(),
        )
        .unwrap()
    };
    assert!(decode(newer) > decode(older));
    assert_eq!(older.next_sequence(), Err(RevisionError::SequenceOverflow));
    assert_eq!(newer.next_sequence().unwrap().sequence(), 1);
    assert!(newer.next_sequence().unwrap() > newer);
    assert_eq!(
        consume(
            fixture::CompatibilityFixture::decode(
                revision_fixture(8, 0).encode_to_vec().as_slice()
            )
            .unwrap()
        ),
        Ok(Payload::Revision(newer))
    );
}

#[test]
fn missing_and_zero_recovery_components_and_old_scalar_ambiguity_reject() {
    assert_eq!(
        read_revision(wire::SessionRevision::default()),
        Err(ContractError::Missing("epoch"))
    );
    assert_eq!(
        read_revision(wire::SessionRevision {
            epoch: Some(wire::RecoveryEpoch::default()),
            sequence: Some(0)
        }),
        Err(ContractError::Missing("epoch"))
    );
    assert_eq!(
        read_revision(wire::SessionRevision {
            epoch: Some(wire::RecoveryEpoch { value: Some(0) }),
            sequence: Some(0)
        }),
        Err(ContractError::Revision(RevisionError::ZeroEpoch))
    );
    assert_eq!(
        read_revision(wire::SessionRevision {
            epoch: Some(wire::RecoveryEpoch { value: Some(1) }),
            sequence: None
        }),
        Err(ContractError::Missing("sequence"))
    );
    // Historical scalar field 1, varint 42: cannot imply a recovery epoch or sequence.
    assert!(wire::SessionRevision::decode(&[8, 42][..]).is_err());
    assert!(wire::SessionRevision::decode(&[10, 5, 8][..]).is_err());
}

#[test]
fn five_required_build_labels_round_trip_without_granting_authority() {
    let identity = BuildIdentity::new(
        Some("source:f79"),
        Some("native/a1"),
        Some("wasm/a1"),
        Some("config.v1"),
        Some("content_unselected-1"),
    )
    .unwrap();
    assert_eq!(
        read_build(
            wire::BuildIdentity::decode(write_build(&identity).encode_to_vec().as_slice()).unwrap()
        ),
        Ok(identity.clone())
    );
    let build = fixture::CompatibilityFixture {
        protocol_revision: Some(1),
        capabilities: vec![],
        payload: Some(fixture::compatibility_fixture::Payload::Build(write_build(
            &identity,
        ))),
    };
    assert_eq!(
        consume(fixture::CompatibilityFixture::decode(build.encode_to_vec().as_slice()).unwrap()),
        Ok(Payload::Build(identity.clone()))
    );
    for (index, revision) in [
        BuildRevision::Source,
        BuildRevision::Native,
        BuildRevision::Wasm,
        BuildRevision::Configuration,
        BuildRevision::Content,
    ]
    .into_iter()
    .enumerate()
    {
        for invalid in [None, Some(""), Some("private secret"), Some("é")] {
            let mut labels = [Some("valid"); 5];
            labels[index] = invalid;
            let error = BuildIdentity::new(labels[0], labels[1], labels[2], labels[3], labels[4])
                .unwrap_err();
            assert_eq!(error.revision, revision);
            let mut supplied = write_build(&identity);
            let value = invalid.map(str::to_owned);
            match revision {
                BuildRevision::Source => supplied.source_revision = value,
                BuildRevision::Native => supplied.native_revision = value,
                BuildRevision::Wasm => supplied.wasm_revision = value,
                BuildRevision::Configuration => supplied.configuration_revision = value,
                BuildRevision::Content => supplied.content_revision = value,
            }
            assert_eq!(
                read_build(
                    wire::BuildIdentity::decode(supplied.encode_to_vec().as_slice()).unwrap()
                ),
                Err(ContractError::Build(error))
            );
        }
    }
    assert_eq!(RevisionLabel::new(None), Err(RevisionLabelError::Missing));
    assert_eq!(RevisionLabel::new(Some("")), Err(RevisionLabelError::Empty));
    assert!(RevisionLabel::new(Some(&"A".repeat(128))).is_ok());
    assert_eq!(
        RevisionLabel::new(Some(&"A".repeat(129))),
        Err(RevisionLabelError::TooLong { actual: 129 })
    );
    assert_eq!(
        RevisionLabel::new(Some("a@b")),
        Err(RevisionLabelError::InvalidCharacter { byte_index: 1 })
    );
    assert_eq!(
        read_build(wire::BuildIdentity::default()),
        Err(ContractError::Build(BuildIdentityError {
            revision: BuildRevision::Source,
            cause: RevisionLabelError::Missing
        }))
    );
}

#[test]
fn older_wire_and_unknown_optional_fields_remain_additive() {
    // Exact older-compatible bytes: protocol 1; revision {epoch {value 1}, sequence 0}.
    let older = [8, 1, 26, 6, 10, 2, 8, 1, 16, 0];
    assert_eq!(
        consume(fixture::CompatibilityFixture::decode(older.as_slice()).unwrap()),
        Ok(Payload::Revision(SessionRevision::new(
            RecoveryEpoch::new(1).unwrap(),
            0
        )))
    );
    let mut additive = older.to_vec();
    additive.extend_from_slice(&[0xA0, 0x06, 0x07]); // unknown optional field100, varint7
    assert_eq!(
        consume(fixture::CompatibilityFixture::decode(additive.as_slice()).unwrap()),
        consume(fixture::CompatibilityFixture::decode(older.as_slice()).unwrap())
    );
}

#[test]
fn unknown_required_capability_zero_enum_missing_oneof_and_protocol_reject() {
    let mut value = revision_fixture(1, 0);
    value.capabilities = vec![fixture::Capability {
        kind: Some(99),
        required: Some(false),
    }];
    assert!(
        consume(fixture::CompatibilityFixture::decode(value.encode_to_vec().as_slice()).unwrap())
            .is_ok()
    );
    value.capabilities[0].required = Some(true);
    assert_eq!(
        consume(fixture::CompatibilityFixture::decode(value.encode_to_vec().as_slice()).unwrap()),
        Err(ContractError::UnknownRequiredCapability(99))
    );
    value.capabilities[0].kind = Some(0);
    assert_eq!(
        consume(value.clone()),
        Err(ContractError::UnspecifiedCapability)
    );
    value.capabilities[0].kind = None;
    assert_eq!(
        consume(value.clone()),
        Err(ContractError::Missing("capability.kind"))
    );
    value.capabilities[0].kind = Some(1);
    value.capabilities[0].required = None;
    assert_eq!(
        consume(value),
        Err(ContractError::Missing("capability.required"))
    );
    for revision in [None, Some(0), Some(2), Some(u32::MAX)] {
        let mut value = revision_fixture(1, 0);
        value.protocol_revision = revision;
        assert_eq!(
            consume(value),
            Err(revision
                .map(ContractError::UnsupportedProtocol)
                .unwrap_or(ContractError::Missing("protocol_revision")))
        );
    }
    let missing = fixture::CompatibilityFixture {
        protocol_revision: Some(1),
        capabilities: vec![],
        payload: None,
    };
    assert_eq!(consume(missing), Err(ContractError::MissingPayload));
    // Future oneof alternative field9 is unknown: decode preserves no known payload.
    assert_eq!(
        consume(fixture::CompatibilityFixture::decode(&[8, 1, 74, 0][..]).unwrap()),
        Err(ContractError::MissingPayload)
    );
    // Known payload plus an unrelated unknown field remains consumable.
    let mut known = revision_fixture(1, 0).encode_to_vec();
    known.extend_from_slice(&[74, 0]);
    assert!(consume(fixture::CompatibilityFixture::decode(known.as_slice()).unwrap()).is_ok());
}
