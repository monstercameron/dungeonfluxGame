#![cfg(not(target_arch = "wasm32"))]

use df_observe::{FixtureTelemetry, OperationContext};

#[test]
fn owned_native_span_exports_the_dispatch_snapshot_after_context_changes() {
    let telemetry = FixtureTelemetry::default();
    let mut context = OperationContext {
        trace_parent: "00-11111111111111111111111111111111-2222222222222222-01".to_owned(),
        build: "admitted-native-build".to_owned(),
    };
    let mut operation = telemetry.begin(&context, "native.owned-operation");
    context.build = "subsequent-build".to_owned();
    context.trace_parent.clear();
    std::thread::spawn(move || operation.finish("complete", 23))
        .join()
        .unwrap();

    assert_eq!(telemetry.count().unwrap(), 1);
    let snapshot = telemetry.snapshot().unwrap();
    assert!(snapshot.starts_with("native.owned-operation trace=11111111111111111111111111111111 "));
    assert!(snapshot.contains("parent=2222222222222222"));
    assert!(snapshot.contains("admitted-native-build"));
    assert!(snapshot.contains("complete"));
    assert!(snapshot.contains("I64(23)"));
    assert!(!snapshot.contains("subsequent-build"));
    assert!(!snapshot.contains("abandoned"));
    telemetry.shutdown().unwrap();
}
