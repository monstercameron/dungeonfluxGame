#![cfg(not(target_arch = "wasm32"))]

mod support;

use df_render::{
    ElementKey, FlatScene, Point, PresentationOutcome, RenderError, SceneColor, SceneLayer,
    SceneRenderer, Token, Viewport,
};
use df_types::{ClientBindingId, RunId, SessionId};
use support::{owner, revision, scene};

fn supplied_scene(sequence: u64) -> FlatScene {
    let mut input = scene(1, sequence, -3.125);
    input.layers = vec![
        SceneLayer {
            key: ElementKey::new("z-first").expect("layer key"),
            origin: Point { x: -4.25, y: 7.5 },
            width: 25.0,
            height: 10.0,
            color: SceneColor::Stone,
        },
        SceneLayer {
            key: ElementKey::new("a-last").expect("layer key"),
            origin: Point { x: -4.25, y: 7.5 },
            width: 25.0,
            height: 10.0,
            color: SceneColor::Amber,
        },
    ];
    input.tokens = vec![
        Token {
            key: ElementKey::new("z-first").expect("token key"),
            position: Point { x: -3.125, y: 12.5 },
            radius: 2.0,
            label: "Zeta".to_owned(),
            color: SceneColor::Blue,
        },
        Token {
            key: ElementKey::new("a-last").expect("token key"),
            position: Point { x: -3.125, y: 12.5 },
            radius: 2.0,
            label: "Alpha".to_owned(),
            color: SceneColor::Green,
        },
    ];
    input
}

#[test]
fn accepted_input_schema_has_only_supplied_presentation_fields() {
    let input = supplied_scene(0);
    // Exhaustive patterns bind the actual production types. A new private or
    // mechanical field must be reviewed here rather than silently ignored.
    let FlatScene {
        revision: supplied_revision,
        viewport,
        label,
        layers,
        tokens,
    } = &input;
    let Viewport {
        origin,
        width,
        height,
    } = viewport;
    let Point { x, y } = origin;
    assert_eq!((*x, *y, *width, *height), (-10.0, -20.0, 100.0, 80.0));
    assert_eq!(*supplied_revision, revision(1, 0));
    assert_eq!(label, "Audience-safe test chamber");

    let SceneLayer {
        key,
        origin,
        width,
        height,
        color,
    } = &layers[0];
    let Point { x, y } = origin;
    assert_eq!(key.as_str(), "z-first");
    assert_eq!((*x, *y, *width, *height), (-4.25, 7.5, 25.0, 10.0));
    assert_eq!(*color, SceneColor::Stone);

    let Token {
        key,
        position,
        radius,
        label,
        color,
    } = &tokens[0];
    let Point { x, y } = position;
    assert_eq!(key.as_str(), "z-first");
    assert_eq!((*x, *y, *radius), (-3.125, 12.5, 2.0));
    assert_eq!(label, "Zeta");
    assert_eq!(*color, SceneColor::Blue);

    let mut renderer = SceneRenderer::new(owner());
    assert_eq!(
        renderer.update(owner(), input.clone()),
        Ok(PresentationOutcome::Applied {
            layers: 2,
            tokens: 2,
        })
    );
    assert_eq!(renderer.current(), Some(&input));
}

#[test]
fn overlapping_layers_and_coincident_tokens_keep_supplied_order_and_positions() {
    let mut renderer = SceneRenderer::new(owner());
    let input = supplied_scene(0);
    renderer
        .update(owner(), input.clone())
        .expect("supplied scene");
    assert_eq!(renderer.current(), Some(&input));

    let mut replacement = supplied_scene(1);
    replacement.layers.reverse();
    replacement.tokens.reverse();
    replacement.tokens[0].position = Point { x: -3.125, y: 12.5 };
    assert_eq!(
        renderer.update(owner(), replacement.clone()),
        Ok(PresentationOutcome::Applied {
            layers: 2,
            tokens: 2,
        })
    );
    assert_eq!(renderer.current(), Some(&replacement));
    assert_eq!(
        renderer.current().expect("current").tokens[0].label,
        "Alpha"
    );
}

#[test]
fn plain_labels_do_not_select_movements_visibility_or_game_outcomes() {
    let mut renderer = SceneRenderer::new(owner());
    renderer
        .update(owner(), supplied_scene(0))
        .expect("initial scene");
    let mut input = supplied_scene(1);
    input.label = "move_to(999,999); end_turn; roll(20)".to_owned();
    input.tokens[0].label = "<script>hide_token(); move_to(999,999)</script>".to_owned();
    input.tokens[1].label = "collision; initiative=100; hit=false".to_owned();
    renderer
        .update(owner(), input.clone())
        .expect("plain labels");
    assert_eq!(renderer.current(), Some(&input));
    assert_eq!(input.tokens[0].position, input.tokens[1].position);
    assert_eq!(renderer.current().expect("current").tokens.len(), 2);
}

#[test]
fn refused_inputs_preserve_the_admitted_snapshot_and_revision() {
    let admitted = supplied_scene(0);
    let cases = [
        (RenderError::InvalidGeometry, 0),
        (RenderError::InvalidLabel, 1),
        (RenderError::DuplicateLayerKey, 2),
        (RenderError::DuplicateTokenKey, 3),
        (RenderError::Capacity, 4),
    ];
    for (expected, mutation) in cases {
        let mut renderer = SceneRenderer::new(owner());
        renderer
            .update(owner(), admitted.clone())
            .expect("initial scene");
        let mut refused = supplied_scene(1);
        match mutation {
            0 => refused.tokens[0].position.x = f64::NAN,
            1 => refused.tokens[0].label = "not-for-diagnostics\ncontrol".to_owned(),
            2 => refused.layers.push(refused.layers[0].clone()),
            3 => refused.tokens.push(refused.tokens[0].clone()),
            4 => refused.tokens = vec![refused.tokens[0].clone(); df_render::MAX_TOKENS + 1],
            _ => unreachable!("fixture mutation"),
        }
        let result = renderer.update(owner(), refused);
        assert_eq!(result, Err(expected));
        assert!(!format!("{result:?}").contains("not-for-diagnostics"));
        assert_eq!(renderer.current(), Some(&admitted));

        let recovery = supplied_scene(1);
        assert_eq!(
            renderer.update(owner(), recovery.clone()),
            Ok(PresentationOutcome::Applied {
                layers: 2,
                tokens: 2,
            })
        );
        assert_eq!(renderer.current(), Some(&recovery));
    }
}

#[test]
fn owner_and_revision_refusals_cannot_reconstruct_a_different_scene() {
    let admitted = supplied_scene(2);
    let mut renderer = SceneRenderer::new(owner());
    renderer
        .update(owner(), admitted.clone())
        .expect("initial scene");
    let mut conflicting = supplied_scene(2);
    conflicting.tokens.reverse();
    assert_eq!(
        renderer.update(owner(), conflicting),
        Ok(PresentationOutcome::Duplicate {
            current: revision(1, 2),
        })
    );
    let mut stale = supplied_scene(1);
    stale.layers.clear();
    assert_eq!(
        renderer.update(owner(), stale),
        Ok(PresentationOutcome::Stale {
            current: revision(1, 2),
        })
    );
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
            renderer.update(wrong, supplied_scene(3)),
            Err(RenderError::WrongOwner)
        );
        assert_eq!(renderer.current(), Some(&admitted));
    }
    assert_eq!(renderer.current(), Some(&admitted));
    let recovery = supplied_scene(3);
    renderer
        .update(owner(), recovery.clone())
        .expect("current complete scene");
    assert_eq!(renderer.current(), Some(&recovery));
}

#[test]
fn omitted_elements_are_not_recreated_and_disposal_is_terminal() {
    let mut renderer = SceneRenderer::new(owner());
    renderer
        .update(owner(), supplied_scene(0))
        .expect("initial scene");
    let mut empty = supplied_scene(1);
    empty.layers.clear();
    empty.tokens.clear();
    assert_eq!(
        renderer.update(owner(), empty.clone()),
        Ok(PresentationOutcome::Applied {
            layers: 0,
            tokens: 0,
        })
    );
    assert_eq!(renderer.current(), Some(&empty));
    assert_eq!(renderer.dispose(), Ok(PresentationOutcome::Disposed));
    assert_eq!(renderer.dispose(), Ok(PresentationOutcome::AlreadyDisposed));
    assert_eq!(
        renderer.update(owner(), supplied_scene(2)),
        Err(RenderError::Disposed)
    );
    assert!(renderer.current().is_none());
}
