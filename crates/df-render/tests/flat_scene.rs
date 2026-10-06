#![cfg(not(target_arch = "wasm32"))]

mod support;

use df_render::{
    ElementKey, MAX_LAYERS, MAX_TOKENS, PresentationOutcome, RenderError, SceneRenderer,
};
use support::{owner, revision, scene};

#[test]
fn accepted_geometry_retains_exact_supplied_positions_and_layer_order() {
    let input = scene(2, 7, -3.125);
    let mut renderer = SceneRenderer::new(owner());
    assert_eq!(
        renderer.update(owner(), input.clone()),
        Ok(PresentationOutcome::Applied {
            layers: 1,
            tokens: 1
        })
    );
    assert_eq!(renderer.current(), Some(&input));
}

#[test]
fn stale_and_conflicting_duplicate_snapshots_cannot_replace_current_positions() {
    let mut renderer = SceneRenderer::new(owner());
    renderer
        .update(owner(), scene(2, 7, 3.0))
        .expect("initial scene");
    assert_eq!(
        renderer.update(owner(), scene(2, 7, 999.0)),
        Ok(PresentationOutcome::Duplicate {
            current: revision(2, 7)
        })
    );
    assert_eq!(
        renderer.update(owner(), scene(1, u64::MAX, 999.0)),
        Ok(PresentationOutcome::Stale {
            current: revision(2, 7)
        })
    );
    assert_eq!(renderer.current(), Some(&scene(2, 7, 3.0)));
    renderer
        .update(owner(), scene(3, 0, 19.5))
        .expect("recovery scene");
    assert_eq!(renderer.current(), Some(&scene(3, 0, 19.5)));
}

#[test]
fn nonfinite_or_out_of_bounds_geometry_does_not_advance_revision() {
    for x in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1_000_001.0] {
        let mut renderer = SceneRenderer::new(owner());
        renderer
            .update(owner(), scene(1, 0, 3.0))
            .expect("initial scene");
        assert_eq!(
            renderer.update(owner(), scene(1, 1, x)),
            Err(RenderError::InvalidGeometry)
        );
        assert_eq!(renderer.current(), Some(&scene(1, 0, 3.0)));
        assert!(renderer.update(owner(), scene(1, 1, 4.0)).is_ok());
    }
}

#[test]
fn duplicate_component_keys_are_rejected_before_snapshot_replacement() {
    let mut renderer = SceneRenderer::new(owner());
    let mut duplicate = scene(1, 0, 3.0);
    duplicate
        .tokens
        .push(duplicate.tokens.first().expect("token").clone());
    assert_eq!(
        renderer.update(owner(), duplicate),
        Err(RenderError::DuplicateTokenKey)
    );
    let mut duplicate = scene(1, 0, 3.0);
    duplicate
        .layers
        .push(duplicate.layers.first().expect("layer").clone());
    assert_eq!(
        renderer.update(owner(), duplicate),
        Err(RenderError::DuplicateLayerKey)
    );
    assert!(renderer.current().is_none());
}

#[test]
fn scene_bounds_reject_excess_tokens_layers_labels_and_invalid_dimensions() {
    let mut renderer = SceneRenderer::new(owner());
    let mut oversized = scene(1, 0, 3.0);
    let token = oversized.tokens.first().expect("token").clone();
    oversized.tokens = vec![token; MAX_TOKENS + 1];
    assert_eq!(
        renderer.update(owner(), oversized),
        Err(RenderError::Capacity)
    );
    let mut oversized = scene(1, 0, 3.0);
    let layer = oversized.layers.first().expect("layer").clone();
    oversized.layers = vec![layer; MAX_LAYERS + 1];
    assert_eq!(
        renderer.update(owner(), oversized),
        Err(RenderError::Capacity)
    );
    let mut oversized = scene(1, 0, 3.0);
    oversized.label = "x".repeat(513);
    assert_eq!(
        renderer.update(owner(), oversized),
        Err(RenderError::InvalidLabel)
    );
    for width in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let mut invalid = scene(1, 0, 3.0);
        invalid.viewport.width = width;
        assert_eq!(
            renderer.update(owner(), invalid),
            Err(RenderError::InvalidGeometry)
        );
    }
    assert_eq!(ElementKey::new(""), Err(RenderError::InvalidKey));
    assert_eq!(
        ElementKey::new("x".repeat(129)),
        Err(RenderError::InvalidKey)
    );
}

#[test]
fn complete_snapshots_remove_omitted_tokens_and_disposal_is_terminal() {
    let mut renderer = SceneRenderer::new(owner());
    for sequence in 0..128 {
        renderer
            .update(owner(), scene(1, sequence, sequence as f64))
            .expect("scene");
        assert_eq!(renderer.current().expect("current").tokens.len(), 1);
    }
    let mut empty = scene(1, 128, 0.0);
    empty.tokens.clear();
    renderer
        .update(owner(), empty)
        .expect("empty permitted snapshot");
    assert!(renderer.current().expect("current").tokens.is_empty());
    assert_eq!(renderer.dispose(), Ok(PresentationOutcome::Disposed));
    assert_eq!(renderer.dispose(), Ok(PresentationOutcome::AlreadyDisposed));
    assert!(!renderer.capabilities().flat_svg);
    assert!(renderer.current().is_none());
    assert_eq!(
        renderer.update(owner(), scene(2, 0, 0.0)),
        Err(RenderError::Disposed)
    );
}

#[test]
fn maximum_distinct_layer_and_token_counts_are_accepted() {
    let mut renderer = SceneRenderer::new(owner());
    let mut bounded = scene(1, 0, 0.0);
    let token = bounded.tokens.first().expect("token").clone();
    bounded.tokens = (0..MAX_TOKENS)
        .map(|index| {
            let mut token = token.clone();
            token.key = ElementKey::new(format!("token-{index}")).expect("key");
            token
        })
        .collect();
    let layer = bounded.layers.first().expect("layer").clone();
    bounded.layers = (0..MAX_LAYERS)
        .map(|index| {
            let mut layer = layer.clone();
            layer.key = ElementKey::new(format!("layer-{index}")).expect("key");
            layer
        })
        .collect();
    assert_eq!(
        renderer.update(owner(), bounded),
        Ok(PresentationOutcome::Applied {
            layers: MAX_LAYERS,
            tokens: MAX_TOKENS
        })
    );
}

#[test]
fn invalid_layer_and_token_shapes_cannot_be_silently_normalized() {
    let mut renderer = SceneRenderer::new(owner());
    let mut invalid = scene(1, 0, 0.0);
    invalid.tokens.first_mut().expect("token").radius = -2.0;
    assert_eq!(
        renderer.update(owner(), invalid),
        Err(RenderError::InvalidGeometry)
    );
    let mut invalid = scene(1, 0, 0.0);
    invalid.layers.first_mut().expect("layer").origin.y = f64::NAN;
    assert_eq!(
        renderer.update(owner(), invalid),
        Err(RenderError::InvalidGeometry)
    );
    let mut invalid = scene(1, 0, 0.0);
    invalid.tokens.first_mut().expect("token").label = "private\ncontrol".to_owned();
    assert_eq!(
        renderer.update(owner(), invalid),
        Err(RenderError::InvalidLabel)
    );
}

#[test]
fn wrong_session_run_or_binding_cannot_replace_admitted_geometry() {
    use df_types::{ClientBindingId, RunId, SessionId};
    let mut renderer = SceneRenderer::new(owner());
    renderer.update(owner(), scene(1, 0, 4.5)).expect("initial");
    for wrong in [
        (
            SessionId::from_bytes(&[8; 16]).expect("session"),
            owner().1,
            owner().2,
        ),
        (
            owner().0,
            RunId::from_bytes(&[8; 16]).expect("run"),
            owner().2,
        ),
        (
            owner().0,
            owner().1,
            ClientBindingId::from_bytes(&[8; 16]).expect("binding"),
        ),
    ] {
        assert_eq!(
            renderer.update(wrong, scene(2, 0, 999.0)),
            Err(RenderError::WrongOwner)
        );
        assert_eq!(renderer.current(), Some(&scene(1, 0, 4.5)));
    }
    assert!(renderer.update(owner(), scene(2, 0, 7.0)).is_ok());
}
