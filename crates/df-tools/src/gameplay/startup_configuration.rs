use std::{env::VarError, io, path::Path};

pub(super) struct StartupAssets {
    pub(super) glue: bytes::Bytes,
    pub(super) wasm: bytes::Bytes,
    pub(super) art: bytes::Bytes,
    pub(super) campfire: bytes::Bytes,
    pub(super) portrait: bytes::Bytes,
    pub(super) inn: Option<bytes::Bytes>,
}

/// This is the local demonstration's existing byte-load policy, not media decoding.
pub(super) fn load_assets(web_root: &Path, asset_root: &Path) -> Result<StartupAssets, io::Error> {
    let glue = super::bounded_asset(web_root.join("df_tools.js"), 1024 * 1024)?;
    let wasm = super::bounded_asset(web_root.join("df_tools_bg.wasm"), 32 * 1024 * 1024)?;
    let art = super::bounded_asset(
        asset_root.join("scenes/mara-harbor-v4.png"),
        16 * 1024 * 1024,
    )?;
    let campfire = super::bounded_asset(
        asset_root.join("../concept-art/scene-campfire-under-stars.webp"),
        4 * 1024 * 1024,
    )?;
    let portrait = super::bounded_asset(
        asset_root.join("../concept-art/vell-avatar.webp"),
        1024 * 1024,
    )?;
    let inn = match super::bounded_asset(
        asset_root.join("../concept-art/scene-tavern-barkeep-talk-rain.webp"),
        4 * 1024 * 1024,
    ) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    Ok(StartupAssets {
        glue,
        wasm,
        art,
        campfire,
        portrait,
        inn,
    })
}

/// Accepts only the current local database-name input; never a credential or endpoint URI.
pub(super) fn database_config(
    input: Result<String, VarError>,
) -> Result<tokio_postgres::Config, io::Error> {
    let name = match input {
        Ok(value) => value,
        Err(VarError::NotPresent) => "df_gameplay_demo_20261004_r03".to_owned(),
        Err(_) => return Err(io::Error::other("local demonstration database invalid")),
    };
    if name.len() > 63
        || !name.starts_with("df_gameplay_demo_")
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(io::Error::other("local demonstration database invalid"));
    }
    let mut configuration = tokio_postgres::Config::new();
    configuration
        .host("127.0.0.1")
        .port(55517)
        .user("df_gameplay_demo_admin_20261004")
        .dbname(&name);
    Ok(configuration)
}
