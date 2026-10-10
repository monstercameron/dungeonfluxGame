#[path = "../src/lib.rs"]
mod server;

use server::deployment::{
    DeploymentMode, DeploymentPolicy, DeploymentRefusal, InstanceId, InstanceIdRefusal,
    OtherStorageRoots, SessionOwner, StorageRoot, TelemetryRootRefusal, TelemetryRoots,
};
use std::path::Path;

fn instance(value: &str) -> InstanceId {
    InstanceId::new(value).unwrap()
}

fn roots(database: &str, spool: &str, segments: &str) -> TelemetryRoots {
    TelemetryRoots {
        instance: instance("server-a"),
        database: database.into(),
        spool: spool.into(),
        segments: segments.into(),
    }
}

fn other_roots() -> OtherStorageRoots<'static> {
    OtherStorageRoots {
        postgresql: Path::new("/srv/postgresql"),
        durable_media: Path::new("/srv/media"),
        workflow_database: Path::new("/srv/workflow/workflow.sqlite3"),
    }
}

#[test]
fn initial_selection_defaults_to_one_process_and_routes_each_session_to_its_actor() {
    let policy = DeploymentPolicy::initial(instance("server-a"));

    assert_eq!(policy.mode(), DeploymentMode::SingleNode);
    let route = policy.route_session();
    assert_eq!(route.instance, policy.instance());
    assert_eq!(route.instance.as_str(), "server-a");
    assert_eq!(route.owner, SessionOwner::ProcessOwnedPerSessionActor);

    let explicitly_selected =
        DeploymentPolicy::select(DeploymentMode::SingleNode, instance("server-b")).unwrap();
    assert_eq!(explicitly_selected.mode(), DeploymentMode::SingleNode);
    assert_eq!(
        explicitly_selected.route_session().instance.as_str(),
        "server-b"
    );
}

#[test]
fn selecting_multi_node_refuses_without_durable_owner_fencing() {
    assert_eq!(
        DeploymentPolicy::select(DeploymentMode::MultiNode, instance("server-a")),
        Err(DeploymentRefusal::MultiNodeUnqualified)
    );
}

#[test]
fn instance_ids_are_bounded_and_path_safe() {
    for (value, expected) in [
        ("", InstanceIdRefusal::Empty),
        (" bad", InstanceIdRefusal::InvalidCharacter),
        ("bad/id", InstanceIdRefusal::InvalidCharacter),
        (
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            InstanceIdRefusal::TooLong,
        ),
    ] {
        assert_eq!(InstanceId::new(value), Err(expected));
    }
    assert_eq!(instance("server_1-a").as_str(), "server_1-a");
}

#[test]
fn telemetry_roots_bind_to_selected_instance_and_are_distinct() {
    let policy = DeploymentPolicy::initial(instance("server-a"));
    let configured = roots(
        "/srv/telemetry/server-a/telemetry.sqlite3",
        "/srv/telemetry/server-a/spool",
        "/srv/telemetry/server-a/segments",
    );
    assert_eq!(
        configured.clone().validate_for(&policy, other_roots()),
        Ok(configured)
    );

    let mismatched = TelemetryRoots {
        instance: instance("server-b"),
        ..roots("/srv/a.db", "/srv/a-spool", "/srv/a-segments")
    };
    assert_eq!(
        mismatched.validate_for(&policy, other_roots()),
        Err(TelemetryRootRefusal::InstanceMismatch)
    );
}

#[test]
fn missing_or_unsafe_roots_are_rejected_for_each_telemetry_role() {
    let policy = DeploymentPolicy::initial(instance("server-a"));
    for (database, spool, segments, expected) in [
        (
            "",
            "/srv/spool",
            "/srv/segments",
            TelemetryRootRefusal::NotAbsolute(StorageRoot::TelemetryDatabase),
        ),
        (
            "/srv/db",
            "",
            "/srv/segments",
            TelemetryRootRefusal::NotAbsolute(StorageRoot::TelemetrySpool),
        ),
        (
            "/srv/db",
            "/srv/spool",
            "",
            TelemetryRootRefusal::NotAbsolute(StorageRoot::TelemetrySegments),
        ),
    ] {
        assert_eq!(
            roots(database, spool, segments).validate_for(&policy, other_roots()),
            Err(expected)
        );
    }

    let traversal = roots("/srv/db", "/srv/spool/../workflow", "/srv/segments");
    assert_eq!(
        traversal.validate_for(&policy, other_roots()),
        Err(TelemetryRootRefusal::ParentTraversal(
            StorageRoot::TelemetrySpool
        ))
    );
}

#[test]
fn each_telemetry_root_rejects_overlap_with_every_other_authority() {
    let policy = DeploymentPolicy::initial(instance("server-a"));
    let telemetry_owners = [
        StorageRoot::TelemetryDatabase,
        StorageRoot::TelemetrySpool,
        StorageRoot::TelemetrySegments,
    ];
    let protected = [
        (StorageRoot::Postgresql, "/srv/postgresql/nested"),
        (StorageRoot::DurableMedia, "/srv/media/nested"),
        (StorageRoot::WorkflowDatabase, "/srv/workflow"),
    ];
    for (index, telemetry_owner) in telemetry_owners.into_iter().enumerate() {
        for (protected_owner, protected_path) in protected {
            let mut paths = [
                "/srv/telemetry/db",
                "/srv/telemetry/spool",
                "/srv/telemetry/segments",
            ];
            paths[index] = protected_path;
            assert_eq!(
                roots(paths[0], paths[1], paths[2]).validate_for(&policy, other_roots()),
                Err(TelemetryRootRefusal::Overlap {
                    first: telemetry_owner,
                    second: protected_owner,
                })
            );
        }
    }

    for (first, second, first_owner, second_owner) in [(
        "/srv/telemetry/same",
        "/srv/telemetry/same",
        StorageRoot::TelemetryDatabase,
        StorageRoot::TelemetrySpool,
    )] {
        assert_eq!(
            roots(first, second, "/srv/other-segments").validate_for(&policy, other_roots()),
            Err(TelemetryRootRefusal::Overlap {
                first: first_owner,
                second: second_owner,
            })
        );
    }

    assert_eq!(
        roots(
            "/srv/telemetry",
            "/srv/other-spool",
            "/srv/telemetry/segments"
        )
        .validate_for(&policy, other_roots()),
        Err(TelemetryRootRefusal::Overlap {
            first: StorageRoot::TelemetryDatabase,
            second: StorageRoot::TelemetrySegments,
        })
    );
}
