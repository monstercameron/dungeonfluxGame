use df_protocol::{common as wire, contract_fixture as fixture};
use df_types::{
    BuildIdentity, BuildIdentityError, BuildRevision, ClientBindingId, Currency, IdentityError,
    LiabilityRate, LocaleTag, LocaleTagError, MemberId, Money, MoneyError, OperationId,
    RecoveryEpoch, RevisionError, RevisionLabel, RevisionLabelError, RunId, SessionId,
    SessionRevision, Usage, UsageUnit,
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
fn recovery_round_trip_preserves_checked_boundaries_and_epoch_first_order() {
    let older = SessionRevision::new(RecoveryEpoch::new(7).unwrap(), u64::MAX);
    let newer = SessionRevision::new(RecoveryEpoch::new(8).unwrap(), 0);
    let maximum_epoch = RecoveryEpoch::new(u64::MAX).unwrap();
    let near_maximum = SessionRevision::new(maximum_epoch, u64::MAX - 1);
    let maximum = near_maximum.next_sequence().unwrap();
    let decode = |value| {
        read_revision(
            wire::SessionRevision::decode(write_revision(value).encode_to_vec().as_slice())
                .unwrap(),
        )
        .unwrap()
    };

    assert!(decode(newer) > decode(older));
    assert_eq!(decode(newer).sequence(), 0);
    assert_eq!(decode(near_maximum), near_maximum);
    assert_eq!(decode(maximum), maximum);
    assert_eq!(
        decode(maximum).next_sequence(),
        Err(RevisionError::SequenceOverflow)
    );
    assert_eq!(
        decode(older).next_sequence(),
        Err(RevisionError::SequenceOverflow)
    );
    assert_eq!(newer.next_sequence().unwrap().sequence(), 1);
    assert!(newer.next_sequence().unwrap() > newer);

    let consumed_near_maximum = consume(
        fixture::CompatibilityFixture::decode(
            revision_fixture(u64::MAX, u64::MAX - 1)
                .encode_to_vec()
                .as_slice(),
        )
        .unwrap(),
    );
    assert_eq!(consumed_near_maximum, Ok(Payload::Revision(near_maximum)));
    let Payload::Revision(consumed_near_maximum) = consumed_near_maximum.unwrap() else {
        unreachable!();
    };
    let consumed_maximum = consumed_near_maximum.next_sequence().unwrap();
    assert_eq!(consumed_maximum, maximum);
    let consumed_maximum = consume(
        fixture::CompatibilityFixture::decode(
            revision_fixture(consumed_maximum.epoch().get(), consumed_maximum.sequence())
                .encode_to_vec()
                .as_slice(),
        )
        .unwrap(),
    );
    assert_eq!(consumed_maximum, Ok(Payload::Revision(maximum)));
    let Payload::Revision(consumed_maximum) = consumed_maximum.unwrap() else {
        unreachable!();
    };
    assert_eq!(
        consumed_maximum.next_sequence(),
        Err(RevisionError::SequenceOverflow)
    );

    let consumed_zero = consume(
        fixture::CompatibilityFixture::decode(revision_fixture(8, 0).encode_to_vec().as_slice())
            .unwrap(),
    );
    assert_eq!(consumed_zero, Ok(Payload::Revision(newer)));
    let Payload::Revision(consumed_zero) = consumed_zero.unwrap() else {
        unreachable!();
    };
    assert_eq!(consumed_zero.next_sequence().unwrap().sequence(), 1);
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

#[test]
fn money_tags_preserve_exact_uppercase_ascii_and_refuse_other_inputs() {
    for tag in ["USD", "EUR", "ZZZ"] {
        let currency = Currency::parse(tag).unwrap();
        assert_eq!(currency.as_str(), tag);
        for micros in [0, 1_000_000, u128::MAX] {
            let amount = Money::new(currency, micros);
            assert_eq!(amount.currency(), currency);
            assert_eq!(amount.currency().as_str(), tag);
            assert_eq!(amount.micros(), micros);
        }
    }
    for tag in [
        "",
        "US",
        "USDD",
        "usd",
        "Usd",
        " USD",
        "US ",
        "U\tD",
        "éA",
        "ＵＳＤ",
    ] {
        assert_eq!(
            Currency::parse(tag),
            Err(MoneyError::InvalidCurrencyTag),
            "{tag:?}"
        );
    }
}

#[test]
fn money_add_sub_preserve_currency_and_refuse_mismatch_before_arithmetic() {
    let usd = Currency::parse("USD").unwrap();
    let eur = Currency::parse("EUR").unwrap();
    for currency in [usd, eur] {
        let zero = Money::new(currency, 0);
        let two = Money::new(currency, 2);
        let three = Money::new(currency, 3);
        let maximum = Money::new(currency, u128::MAX);
        assert_eq!(two.checked_add(three), Ok(Money::new(currency, 5)));
        assert_eq!(three.checked_sub(two), Ok(Money::new(currency, 1)));
        assert_eq!(two.checked_sub(two), Ok(zero));
        assert_eq!(zero.checked_add(zero), Ok(zero));
        assert_eq!(zero.checked_sub(zero), Ok(zero));
        assert_eq!(maximum.checked_add(zero), Ok(maximum));
        assert_eq!(maximum.checked_sub(zero), Ok(maximum));
        assert_eq!(
            Money::new(currency, u128::MAX - 1).checked_add(Money::new(currency, 1)),
            Ok(maximum)
        );
        assert_eq!(
            maximum.checked_add(Money::new(currency, 1)),
            Err(MoneyError::Overflow)
        );
        assert_eq!(two.checked_sub(three), Err(MoneyError::Underflow));
    }
    for (left, right) in [(usd, eur), (eur, usd)] {
        assert_eq!(
            Money::new(left, 1).checked_add(Money::new(right, 1)),
            Err(MoneyError::CurrencyMismatch)
        );
        assert_eq!(
            Money::new(left, u128::MAX).checked_add(Money::new(right, 1)),
            Err(MoneyError::CurrencyMismatch)
        );
        assert_eq!(
            Money::new(left, 1).checked_sub(Money::new(right, 0)),
            Err(MoneyError::CurrencyMismatch)
        );
        assert_eq!(
            Money::new(left, 0).checked_sub(Money::new(right, 1)),
            Err(MoneyError::CurrencyMismatch)
        );
    }
}

#[test]
fn money_usage_preserves_every_unit_and_refuses_mismatch_before_overflow() {
    let units = [
        UsageUnit::Token,
        UsageUnit::Character,
        UsageUnit::Byte,
        UsageUnit::AudioMillisecond,
        UsageUnit::VideoMillisecond,
        UsageUnit::Image,
    ];
    for unit in units {
        for quantity in [0, u128::MAX] {
            let usage = Usage::new(quantity, unit);
            assert_eq!(usage.quantity(), quantity);
            assert_eq!(usage.unit(), unit);
            assert_eq!(usage.checked_add(Usage::new(0, unit)), Ok(usage));
        }
        assert_eq!(
            Usage::new(2, unit).checked_add(Usage::new(3, unit)),
            Ok(Usage::new(5, unit))
        );
        assert_eq!(
            Usage::new(u128::MAX - 1, unit).checked_add(Usage::new(1, unit)),
            Ok(Usage::new(u128::MAX, unit))
        );
        assert_eq!(
            Usage::new(u128::MAX, unit).checked_add(Usage::new(1, unit)),
            Err(MoneyError::Overflow)
        );
        for other in units {
            if other != unit {
                assert_eq!(
                    Usage::new(1, unit).checked_add(Usage::new(1, other)),
                    Err(MoneyError::UnitMismatch)
                );
                assert_eq!(
                    Usage::new(u128::MAX, unit).checked_add(Usage::new(1, other)),
                    Err(MoneyError::UnitMismatch)
                );
            }
        }
    }
}

#[test]
fn money_liability_rounds_up_exactly_and_preserves_rate_accessors() {
    let usd = Currency::parse("USD").unwrap();
    let eur = Currency::parse("EUR").unwrap();
    for currency in [usd, eur] {
        for unit in [
            UsageUnit::Token,
            UsageUnit::Character,
            UsageUnit::Byte,
            UsageUnit::AudioMillisecond,
            UsageUnit::VideoMillisecond,
            UsageUnit::Image,
        ] {
            for (quantity, numerator, denominator, expected) in [
                (5, 2, 3, 4),
                (6, 2, 3, 4),
                (1, 1, 2, 1),
                (0, u128::MAX, 3, 0),
                (u128::MAX, 0, 1, 0),
                (u128::MAX, 1, 1, u128::MAX),
                (1, u128::MAX, 2, u128::MAX / 2 + 1),
            ] {
                let rate = LiabilityRate::new(currency, unit, numerator, denominator).unwrap();
                assert_eq!(rate.currency(), currency);
                assert_eq!(rate.unit(), unit);
                assert_eq!(rate.numerator_micros(), numerator);
                assert_eq!(rate.denominator_usage_units(), denominator);
                let liability = rate.liability(Usage::new(quantity, unit)).unwrap();
                assert_eq!(liability.currency().as_str(), currency.as_str());
                assert_eq!(liability.micros(), expected);
                assert_eq!(liability, Money::new(currency, expected));
            }
        }
    }
}

#[test]
fn money_rate_refuses_zero_denominator_unit_mismatch_and_intermediate_overflow() {
    let usd = Currency::parse("USD").unwrap();
    for numerator in [0, 1, u128::MAX] {
        // Construction refuses the invalid rate before any zero usage could be applied.
        assert_eq!(
            LiabilityRate::new(usd, UsageUnit::Token, numerator, 0),
            Err(MoneyError::ZeroDenominator)
        );
    }
    let units = [
        UsageUnit::Token,
        UsageUnit::Character,
        UsageUnit::Byte,
        UsageUnit::AudioMillisecond,
        UsageUnit::VideoMillisecond,
        UsageUnit::Image,
    ];
    for unit in units {
        let rate = LiabilityRate::new(usd, unit, 2, 3).unwrap();
        // The mathematical quotient fits, but the bounded intermediate product does not.
        assert_eq!(
            rate.liability(Usage::new(u128::MAX, unit)),
            Err(MoneyError::Overflow)
        );
        let maximum_rate = LiabilityRate::new(usd, unit, u128::MAX, u128::MAX).unwrap();
        assert_eq!(
            maximum_rate.liability(Usage::new(2, unit)),
            Err(MoneyError::Overflow)
        );
        for other in units {
            if other != unit {
                assert_eq!(
                    rate.liability(Usage::new(1, other)),
                    Err(MoneyError::UnitMismatch)
                );
                assert_eq!(
                    rate.liability(Usage::new(u128::MAX, other)),
                    Err(MoneyError::UnitMismatch)
                );
                assert_eq!(
                    rate.liability(Usage::new(0, other)),
                    Err(MoneyError::UnitMismatch)
                );
            }
        }
    }
}

#[test]
fn locale_preferences_execute_the_127_frozen_public_contract_expectations() {
    use LocaleTagError::{DuplicateExtension, DuplicateVariant, Empty, InvalidSyntax, TooLong};

    // Frozen I04 admission cases exercise this external consumer's actual df-types API.
    // These are spelling/error expectations, not a second parser or support catalog.
    let cases: [(&str, Result<&str, LocaleTagError>); 127] = [
        ("en", Ok("en")),
        ("EN-us", Ok("en-us")),
        ("sr-Latn-RS", Ok("sr-latn-rs")),
        ("es-419", Ok("es-419")),
        ("abcd", Ok("abcd")),
        ("abcdefgh", Ok("abcdefgh")),
        ("en-abc", Ok("en-abc")),
        ("en-abc-def", Ok("en-abc-def")),
        ("en-abc-def-ghi", Ok("en-abc-def-ghi")),
        ("zh-cmn-Hans-CN", Ok("zh-cmn-hans-cn")),
        ("en-1234", Ok("en-1234")),
        ("en-12345", Ok("en-12345")),
        ("en-abcde", Ok("en-abcde")),
        ("en-abcdefgh", Ok("en-abcdefgh")),
        ("sl-rozaj-biske-1994", Ok("sl-rozaj-biske-1994")),
        ("de-CH-1901-1996", Ok("de-ch-1901-1996")),
        ("en-a-aa", Ok("en-a-aa")),
        ("en-0-aa", Ok("en-0-aa")),
        ("en-9-12345678", Ok("en-9-12345678")),
        ("en-z-zz", Ok("en-z-zz")),
        ("en-b-bb-a-aa", Ok("en-b-bb-a-aa")),
        ("en-a-AA-aa", Ok("en-a-aa-aa")),
        ("en-a-abcde-abcde", Ok("en-a-abcde-abcde")),
        ("en-a-bb-x-a", Ok("en-a-bb-x-a")),
        ("en-a-aa-x-a-aa", Ok("en-a-aa-x-a-aa")),
        ("x-a", Ok("x-a")),
        ("X-Private", Ok("x-private")),
        ("en-x-a", Ok("en-x-a")),
        ("en-x-a-a-x-x", Ok("en-x-a-a-x-x")),
        ("x-abcde-ABCDE", Ok("x-abcde-abcde")),
        ("en-GB-oed", Ok("en-gb-oed")),
        ("EN-GB-OED", Ok("en-gb-oed")),
        ("i-ami", Ok("i-ami")),
        ("I-AMI", Ok("i-ami")),
        ("i-bnn", Ok("i-bnn")),
        ("I-BNN", Ok("i-bnn")),
        ("i-default", Ok("i-default")),
        ("I-DEFAULT", Ok("i-default")),
        ("i-enochian", Ok("i-enochian")),
        ("I-ENOCHIAN", Ok("i-enochian")),
        ("i-hak", Ok("i-hak")),
        ("I-HAK", Ok("i-hak")),
        ("i-klingon", Ok("i-klingon")),
        ("I-KLINGON", Ok("i-klingon")),
        ("i-lux", Ok("i-lux")),
        ("I-LUX", Ok("i-lux")),
        ("i-mingo", Ok("i-mingo")),
        ("I-MINGO", Ok("i-mingo")),
        ("i-navajo", Ok("i-navajo")),
        ("I-NAVAJO", Ok("i-navajo")),
        ("i-pwn", Ok("i-pwn")),
        ("I-PWN", Ok("i-pwn")),
        ("i-tao", Ok("i-tao")),
        ("I-TAO", Ok("i-tao")),
        ("i-tay", Ok("i-tay")),
        ("I-TAY", Ok("i-tay")),
        ("i-tsu", Ok("i-tsu")),
        ("I-TSU", Ok("i-tsu")),
        ("sgn-BE-FR", Ok("sgn-be-fr")),
        ("SGN-BE-FR", Ok("sgn-be-fr")),
        ("sgn-BE-NL", Ok("sgn-be-nl")),
        ("SGN-BE-NL", Ok("sgn-be-nl")),
        ("sgn-CH-DE", Ok("sgn-ch-de")),
        ("SGN-CH-DE", Ok("sgn-ch-de")),
        ("art-lojban", Ok("art-lojban")),
        ("ART-LOJBAN", Ok("art-lojban")),
        ("cel-gaulish", Ok("cel-gaulish")),
        ("CEL-GAULISH", Ok("cel-gaulish")),
        ("no-bok", Ok("no-bok")),
        ("NO-BOK", Ok("no-bok")),
        ("no-nyn", Ok("no-nyn")),
        ("NO-NYN", Ok("no-nyn")),
        ("zh-guoyu", Ok("zh-guoyu")),
        ("ZH-GUOYU", Ok("zh-guoyu")),
        ("zh-hakka", Ok("zh-hakka")),
        ("ZH-HAKKA", Ok("zh-hakka")),
        ("zh-min", Ok("zh-min")),
        ("ZH-MIN", Ok("zh-min")),
        ("zh-min-nan", Ok("zh-min-nan")),
        ("ZH-MIN-NAN", Ok("zh-min-nan")),
        ("zh-xiang", Ok("zh-xiang")),
        ("ZH-XIANG", Ok("zh-xiang")),
        ("e", Err(InvalidSyntax)),
        ("abcdefghi", Err(InvalidSyntax)),
        ("a-value", Err(InvalidSyntax)),
        ("en_Us", Err(InvalidSyntax)),
        (" en", Err(InvalidSyntax)),
        ("en ", Err(InvalidSyntax)),
        ("en\tUS", Err(InvalidSyntax)),
        ("en\n", Err(InvalidSyntax)),
        ("en--US", Err(InvalidSyntax)),
        ("-en", Err(InvalidSyntax)),
        ("en-", Err(InvalidSyntax)),
        ("en-ä", Err(InvalidSyntax)),
        ("en–US", Err(InvalidSyntax)),
        ("en-ＵＳ", Err(InvalidSyntax)),
        ("en-US-Latn", Err(InvalidSyntax)),
        ("en-US-GB", Err(InvalidSyntax)),
        ("en-Latn-Latn", Err(InvalidSyntax)),
        ("en-abc-def-ghi-jkl", Err(InvalidSyntax)),
        ("abcd-efg", Err(InvalidSyntax)),
        ("en-abc123456", Err(InvalidSyntax)),
        ("en-12", Err(InvalidSyntax)),
        ("en-a", Err(InvalidSyntax)),
        ("en-a-b-aa", Err(InvalidSyntax)),
        ("en-a-a", Err(InvalidSyntax)),
        ("en-a-abcdefghi", Err(InvalidSyntax)),
        ("en-x", Err(InvalidSyntax)),
        ("x", Err(InvalidSyntax)),
        ("x-", Err(InvalidSyntax)),
        ("x-abcdefghi", Err(InvalidSyntax)),
        ("i-notreal", Err(InvalidSyntax)),
        ("en-GB-oed-x-a", Err(InvalidSyntax)),
        ("sgn-BE-FR-x-a", Err(InvalidSyntax)),
        ("en-abcde-abcde-a", Err(InvalidSyntax)),
        ("en-a-aa-a", Err(InvalidSyntax)),
        ("", Err(Empty)),
        ("en-abcde-ABCDE", Err(DuplicateVariant)),
        ("de-1901-1901", Err(DuplicateVariant)),
        ("en-a-aa-A-bb", Err(DuplicateExtension)),
        ("en-0-aa-0-bb", Err(DuplicateExtension)),
        ("en-abcde-ABCDE-a-aa-A-bb", Err(DuplicateVariant)),
        (
            "x-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcde",
            Ok(
                "x-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcde",
            ),
        ),
        (
            "x-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdefg-abcdef",
            Err(TooLong { actual: 256 }),
        ),
        (
            "                                                                                                                                                                                                                                                                ",
            Err(TooLong { actual: 256 }),
        ),
        (
            "éééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééé",
            Err(TooLong { actual: 256 }),
        ),
        ("en-123", Ok("en-123")),
    ];
    for (input, expected) in cases {
        for _ in 0..3 {
            let parsed = LocaleTag::parse(input);
            assert_eq!(
                parsed
                    .as_ref()
                    .map(LocaleTag::as_str)
                    .map_err(|error| *error),
                expected,
                "{input:?}"
            );
            match parsed {
                Ok(tag) => {
                    assert_eq!(tag, tag.clone());
                    assert_eq!(LocaleTag::parse(tag.as_str()), Ok(tag.clone()));
                    assert_eq!(LocaleTag::parse(&input.to_ascii_uppercase()), Ok(tag));
                }
                Err(error) => {
                    // Matching typed facts requires no access to rejected payloads.
                    match error {
                        Empty | InvalidSyntax | DuplicateVariant | DuplicateExtension => {}
                        TooLong { actual } => assert_eq!(actual, input.len()),
                    }
                    let copied = error;
                    assert_eq!(error, copied);
                }
            }
        }
    }
}

#[test]
fn locale_preferences_preserve_identity_without_aliasing_reordering_or_fallback() {
    assert_eq!(LocaleTag::MAX_BYTES, 255);
    for (left, right) in [
        ("iw", "he"),
        ("i-klingon", "tlh"),
        ("en-b-bb-a-aa", "en-a-aa-b-bb"),
    ] {
        assert_ne!(
            LocaleTag::parse(left).unwrap(),
            LocaleTag::parse(right).unwrap()
        );
    }
    for input in [
        "qqq-Zzzz-ZZ",
        "en-a-abcde-abcde",
        "x-abcde-ABCDE",
        "en-x-a-a-x-x",
    ] {
        assert_eq!(
            LocaleTag::parse(input).unwrap().as_str(),
            input.to_ascii_lowercase()
        );
    }
    let supplied = String::from("SR-Latn-RS");
    let tag = LocaleTag::parse(&supplied).unwrap();
    drop(supplied);
    assert_eq!(tag.as_str(), "sr-latn-rs");
    assert_eq!(
        LocaleTag::parse("en_US"),
        Err(LocaleTagError::InvalidSyntax)
    );
    assert_eq!(LocaleTag::parse(""), Err(LocaleTagError::Empty));
}
