use df_render::{
    ElementKey, FlatScene, Point, SceneColor, SceneLayer, SceneOwner, Token, Viewport,
};
use df_types::{ClientBindingId, RecoveryEpoch, RunId, SessionId, SessionRevision};

pub fn owner() -> SceneOwner {
    (
        SessionId::from_bytes(&[1; 16]).expect("session"),
        RunId::from_bytes(&[2; 16]).expect("run"),
        ClientBindingId::from_bytes(&[3; 16]).expect("binding"),
    )
}

pub fn revision(epoch: u64, sequence: u64) -> SessionRevision {
    SessionRevision::new(RecoveryEpoch::new(epoch).expect("fixture epoch"), sequence)
}

pub fn scene(epoch: u64, sequence: u64, x: f64) -> FlatScene {
    FlatScene {
        revision: revision(epoch, sequence),
        viewport: Viewport {
            origin: Point { x: -10.0, y: -20.0 },
            width: 100.0,
            height: 80.0,
        },
        label: "Audience-safe test chamber".to_owned(),
        layers: vec![SceneLayer {
            key: ElementKey::new("floor").expect("fixture key"),
            origin: Point { x: -4.25, y: 7.5 },
            width: 25.0,
            height: 10.0,
            color: SceneColor::Slate,
        }],
        tokens: vec![Token {
            key: ElementKey::new("permitted-token").expect("fixture key"),
            position: Point { x, y: 12.5 },
            radius: 2.0,
            label: "Ari <script> & friends".to_owned(),
            color: SceneColor::Blue,
        }],
    }
}
