#![cfg(target_arch = "wasm32")]
use df_tools::public_scene_delivery_fixture as actual_client;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn mount() -> Result<(), JsValue> {
    actual_client::mount()
}
#[wasm_bindgen]
pub fn present(
    epoch: u32,
    sequence: u32,
    tavern: bool,
    display: bool,
    wrong_scope: bool,
    old_generation: bool,
) -> Result<(), JsValue> {
    actual_client::present(
        epoch,
        sequence,
        tavern,
        display,
        wrong_scope,
        old_generation,
    )
}
#[wasm_bindgen]
pub fn reconnect() -> Result<(), JsValue> {
    actual_client::reconnect()
}
#[wasm_bindgen]
pub fn replace_scope(binding: u8) -> Result<(), JsValue> {
    actual_client::replace_scope(binding)
}
#[wasm_bindgen]
pub fn counts() -> Result<u32, JsValue> {
    actual_client::counts()
}
#[wasm_bindgen]
pub fn failure() -> Result<String, JsValue> {
    actual_client::failure()
}
#[wasm_bindgen]
pub fn dispose() -> Result<(), JsValue> {
    actual_client::dispose()
}
