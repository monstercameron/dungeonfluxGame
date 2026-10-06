use df_render::{ElementKey, FlatScene, Point, SceneColor, Token, Viewport};
use df_types::SessionRevision;

pub fn flat(revision: SessionRevision) -> FlatScene {
    FlatScene {
        revision,
        viewport: Viewport {
            origin: Point { x: -2.0, y: -3.0 },
            width: 20.0,
            height: 15.0,
        },
        label: "Current resource consumer".to_owned(),
        layers: vec![],
        tokens: vec![Token {
            key: ElementKey::new("same-current-token").unwrap(),
            position: Point { x: 4.0, y: 5.0 },
            radius: 1.0,
            label: "Supplied label".to_owned(),
            color: SceneColor::Blue,
        }],
    }
}
