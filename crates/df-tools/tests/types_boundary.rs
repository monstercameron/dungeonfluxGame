#![cfg(not(target_arch = "wasm32"))]

#[path = "../../df-types/contracts/boundary.rs"]
mod boundary;

use boundary::{
    AssetId, ClientId, Generation, GenerationError, RulesetComponent, RulesetId, UtteranceId,
};
use df_assets::{ByteRange, RangeError};
use df_client::connection::{CallOutcome, ConnectionGeneration, RpcConnection, RpcError};
use df_model::checkpoint::{
    AssetKind, AssetReference, Basis, ContentDigest, JobId, RulesMode, RulesPins,
};
use df_protocol::common as wire;
use df_types::{
    ClientBindingId, IdentityError, OperationId, RecoveryEpoch, RevisionLabel, RevisionLabelError,
    RunId, SessionId, SessionRevision, TextIdentityError,
};
use std::future::{Future, pending};
use std::task::{Context, Poll};

// Candidate associations freeze exact current consumer fields. They are fixture-only values,
// not public service DTOs, authorization capabilities or a production state writer.
struct ClientAssociation {
    client: ClientId,
    binding: ClientBindingId,
    connection: ConnectionGeneration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct UtteranceAssociation {
    utterance: UtteranceId,
    basis: Basis,
    job: JobId,
    operation: OperationId,
    generation: Generation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AssetAssociation {
    asset: AssetId,
    reference: AssetReference,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RulesetAssociation {
    ruleset: RulesetId,
    pins: RulesPins,
}

fn label(value: &str) -> RevisionLabel {
    RevisionLabel::new(Some(value)).unwrap()
}

fn basis() -> Basis {
    Basis {
        session: SessionId::from_bytes(&[1; 16]).unwrap(),
        run: RunId::from_bytes(&[2; 16]).unwrap(),
        revision: SessionRevision::new(RecoveryEpoch::new(3).unwrap(), 4),
    }
}

#[test]
fn canonical_job_and_candidate_utterance_keep_distinct_roles_and_complete_current_scope() {
    let bytes = [5; 16];
    let utterance = UtteranceId::from_bytes(&bytes).unwrap();
    assert_eq!(
        UtteranceId::from_hex(&"05".repeat(16)).unwrap().as_bytes(),
        &bytes
    );
    let job = JobId::from_bytes(&bytes).unwrap();
    assert_eq!(job.as_bytes(), &bytes);
    assert_eq!(JobId::from_bytes(&[0; 16]), Err(IdentityError::Zero));
    assert_eq!(
        JobId::from_bytes(&[1; 15]),
        Err(IdentityError::InvalidLength { actual: 15 })
    );
    assert_eq!(UtteranceId::from_bytes(&[0; 16]), Err(IdentityError::Zero));
    let original = UtteranceAssociation {
        utterance,
        basis: basis(),
        job,
        operation: OperationId::from_bytes(&[6; 16]).unwrap(),
        generation: Generation::new(7).unwrap(),
    };
    let mut changed = original.clone();
    changed.generation = original.generation.next().unwrap();
    assert_ne!(original, changed);
    assert_eq!(original.generation.get(), 7);
    changed = original.clone();
    changed.basis.run = RunId::from_bytes(&[8; 16]).unwrap();
    assert_ne!(original, changed);
    changed = original.clone();
    changed.operation = OperationId::from_bytes(&[9; 16]).unwrap();
    assert_ne!(original, changed);
    assert_eq!(Generation::new(0), Err(GenerationError::Zero));
    assert_eq!(
        Generation::new(u64::MAX).unwrap().next(),
        Err(GenerationError::Exhausted)
    );
    // The real SpeechScheduler/MediaLifecycle suites qualify current completion and buffer
    // ownership. This association alone does not prove a job is admitted or completed.
}

#[test]
fn candidate_asset_identity_preserves_the_entire_current_reference_without_becoming_access() {
    let asset = AssetId::from_bytes(&[10; 16]).unwrap();
    assert_eq!(
        AssetId::from_hex(&"0a".repeat(16)).unwrap().as_bytes(),
        &[10; 16]
    );
    assert_eq!(AssetId::from_bytes(&[0; 16]), Err(IdentityError::Zero));
    let original = AssetAssociation {
        asset,
        reference: AssetReference {
            key: label("immutable-asset-v1"),
            digest: ContentDigest([11; 32]),
            byte_length: 12,
            kind: AssetKind::TacticalGeometry,
        },
    };
    let mut changed = original.clone();
    changed.reference.digest = ContentDigest([12; 32]);
    assert_ne!(original, changed);
    changed = original.clone();
    changed.reference.key = label("immutable-asset-v2");
    assert_ne!(original, changed);
    changed = original.clone();
    changed.reference.byte_length += 1;
    assert_ne!(original, changed);
    assert_eq!(original.asset, changed.asset);
    assert!(matches!(
        ByteRange::from_start_len(u64::MAX, 1),
        Err(RangeError::InvalidRange)
    ));
    assert!(matches!(
        ByteRange::new(1, 1),
        Err(RangeError::InvalidRange)
    ));
    let range = ByteRange::from_start_len(0, original.reference.byte_length).unwrap();
    assert_eq!(range.end(), 12);
    // Actual AssetResolver/AuthorizedRange publication, current-rights refusal and reader
    // cleanup are exercised by the required unchanged df-assets suites, not this supplied ID.
}

#[test]
fn candidate_ruleset_association_preserves_mode_all_digests_and_handler_in_current_rules_pins() {
    let ruleset =
        RulesetId::new(Some("standard-2024"), Some("catalog-v1"), Some("source-v1")).unwrap();
    let original = RulesetAssociation {
        pins: RulesPins {
            mode: RulesMode::Standard2024,
            ruleset: ruleset.mechanics().clone(),
            catalog: ruleset.catalog().clone(),
            catalog_digest: ContentDigest([13; 32]),
            source_manifest: ruleset.source_manifest().clone(),
            source_manifest_digest: ContentDigest([14; 32]),
            handler: label("handler-v1"),
            handler_digest: ContentDigest([15; 32]),
        },
        ruleset,
    };
    for field in 0..4 {
        let mut changed = original.clone();
        match field {
            0 => changed.pins.catalog_digest = ContentDigest([16; 32]),
            1 => changed.pins.source_manifest_digest = ContentDigest([16; 32]),
            2 => changed.pins.handler_digest = ContentDigest([16; 32]),
            3 => changed.pins.mode = RulesMode::DisclosedCustom,
            _ => unreachable!(),
        }
        assert_eq!(original.ruleset, changed.ruleset);
        assert_ne!(original, changed);
    }
    let error = RulesetId::new(Some("standard-2024"), None, Some("source-v1")).unwrap_err();
    assert_eq!(error.component, RulesetComponent::Catalog);
    assert_eq!(error.cause, RevisionLabelError::Missing);
}

#[test]
fn candidate_client_and_stable_binding_cannot_keep_a_closed_or_recreated_connection_current() {
    let connection = RpcConnection::new(());
    let original = ClientAssociation {
        client: ClientId::from_bytes(&[17; 16]).unwrap(),
        binding: ClientBindingId::from_bytes(&[18; 16]).unwrap(),
        connection: connection.generation(),
    };
    assert_eq!(
        ClientId::from_hex(&"11".repeat(16)).unwrap().as_bytes(),
        &[17; 16]
    );
    assert_eq!(ClientId::from_bytes(&[0; 16]), Err(IdentityError::Zero));
    assert_eq!(
        ClientId::from_hex(""),
        Err(TextIdentityError::InvalidLength { actual: 0 })
    );
    assert!(original.connection.same_scope(&connection.generation()));
    assert!(original.connection.is_active());
    connection.close();
    assert!(!original.connection.is_active());
    let newer = RpcConnection::new(());
    let recovered = ClientAssociation {
        client: original.client,
        binding: original.binding,
        connection: newer.generation(),
    };
    assert_eq!(original.client, recovered.client);
    assert_eq!(original.binding, recovered.binding);
    assert!(!original.connection.same_scope(&recovered.connection));
    assert!(recovered.connection.is_active());
    drop(newer);
    assert!(!recovered.connection.is_active());
}

#[test]
fn actual_client_wait_owner_reports_disposal_and_close_without_mutation_acknowledgement() {
    let connection = RpcConnection::new(());
    let waker = futures::task::noop_waker();
    let mut context = Context::from_waker(&waker);
    let (old_cancel, old_wait) = connection
        .unary(async |_client: &mut ()| {
            pending::<Result<tonic::Response<wire::SessionId>, tonic::Status>>().await
        })
        .unwrap();
    let mut old_wait = Box::pin(old_wait);
    assert!(old_wait.as_mut().poll(&mut context).is_pending());
    drop(old_wait);
    assert_eq!(old_cancel.outcome(), CallOutcome::WaitDisposed);
    let (current_cancel, current_wait) = connection
        .unary(async |_client: &mut ()| {
            pending::<Result<tonic::Response<wire::SessionId>, tonic::Status>>().await
        })
        .unwrap();
    let mut current_wait = Box::pin(current_wait);
    assert!(current_wait.as_mut().poll(&mut context).is_pending());
    old_cancel.cancel();
    assert_eq!(old_cancel.outcome(), CallOutcome::WaitDisposed);
    assert_eq!(current_cancel.outcome(), CallOutcome::Waiting);
    connection.close();
    assert!(matches!(
        current_wait.as_mut().poll(&mut context),
        Poll::Ready(Err(RpcError::ConnectionClosed))
    ));
    assert_eq!(current_cancel.outcome(), CallOutcome::ConnectionClosed);
    assert!(!connection.generation().is_active());
}
