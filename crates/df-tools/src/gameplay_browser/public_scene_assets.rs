//! Closed public-demo catalogue and the actual campaign byte-delivery owner.
//! These source-owned public files grant no generated/private asset permission.
use df_client::cache::{CacheKey, CacheScope};
use df_types::{RevisionLabel, SessionRevision};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PublicSceneKind {
    Harbor,
    Tavern,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct PublicSceneAsset {
    pub(crate) path: &'static str,
    pub(crate) kind: PublicSceneKind,
    pub(crate) byte_len: u64,
    pub(crate) sha256: [u8; 32],
    version: &'static str,
}
// Exact source-owned public-file bytes; Root seals both files in the source closure.
const HARBOR: PublicSceneAsset = PublicSceneAsset {
    path: "/assets/ui/scenes/mara-harbor-v4.png",
    kind: PublicSceneKind::Harbor,
    byte_len: 2_316_859,
    sha256: [
        0x49, 0x83, 0xbe, 0xc2, 0x1d, 0x78, 0x7c, 0x40, 0x67, 0x68, 0x6a, 0x9a, 0x44, 0x00, 0xec,
        0x5e, 0x05, 0xb7, 0xc3, 0xdd, 0xaf, 0xb6, 0xf5, 0xa6, 0xe1, 0x88, 0x71, 0xc6, 0xf7, 0x76,
        0x5e, 0xa2,
    ],
    version: "public-demo-harbor-v4-4983bec21d787c4067686a9a4400ec5e",
};
const TAVERN: PublicSceneAsset = PublicSceneAsset {
    path: "/assets/concept-art/scene-tavern-barkeep-talk-rain.webp",
    kind: PublicSceneKind::Tavern,
    byte_len: 249_416,
    sha256: [
        0x3a, 0xd4, 0x1b, 0xf7, 0xfe, 0xeb, 0xe0, 0x07, 0x02, 0xb5, 0x7a, 0x83, 0x8e, 0x11, 0x77,
        0x56, 0xf4, 0x71, 0x8d, 0x92, 0x05, 0xcb, 0xb1, 0x09, 0x4a, 0x97, 0xab, 0x9e, 0xc9, 0xa4,
        0xec, 0xd8,
    ],
    version: "public-demo-tavern-3ad41bf7feebe00702b57a838e117756",
};
#[derive(Debug)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Clone, Copy, PartialEq, Eq))]
pub(crate) enum PublicSceneError {
    UnknownAsset,
    InvalidKey,
    StaleRevision,
    ConflictingSnapshot,
    #[cfg(target_arch = "wasm32")]
    Closed,
    #[cfg(target_arch = "wasm32")]
    Browser,
    #[cfg(target_arch = "wasm32")]
    HttpStatus(u16),
    #[cfg(target_arch = "wasm32")]
    MissingLength,
    #[cfg(target_arch = "wasm32")]
    LengthMismatch,
    #[cfg(target_arch = "wasm32")]
    Oversized,
    #[cfg(target_arch = "wasm32")]
    Transport,
    #[cfg(target_arch = "wasm32")]
    Timeout,
    #[cfg(target_arch = "wasm32")]
    Cancelled,
    #[cfg(target_arch = "wasm32")]
    Campaign(df_ui::CampaignError),
}
impl std::fmt::Display for PublicSceneError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(target_arch = "wasm32")]
            Self::Campaign(error) => {
                write!(formatter, "public scene presentation refused: {error}")
            }
            #[cfg(target_arch = "wasm32")]
            Self::HttpStatus(status) => write!(formatter, "public scene HTTP status {status}"),
            error => write!(formatter, "public scene delivery refused: {error:?}"),
        }
    }
}
pub(crate) fn resolve(path: &str) -> Result<PublicSceneAsset, PublicSceneError> {
    match path {
        "assets/ui/scenes/mara-harbor-v4.png" => Ok(HARBOR),
        "assets/concept-art/scene-tavern-barkeep-talk-rain.webp" => Ok(TAVERN),
        _ => Err(PublicSceneError::UnknownAsset),
    }
}
impl PublicSceneAsset {
    pub(crate) fn key(self) -> Result<CacheKey, PublicSceneError> {
        Ok(CacheKey {
            version: RevisionLabel::new(Some(self.version))
                .map_err(|_| PublicSceneError::InvalidKey)?,
            bytes: df_assets::AssetManifest {
                byte_len: self.byte_len,
                sha256: self.sha256,
            },
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Admission {
    New,
    Retained,
    Replaced,
}
pub(crate) struct SceneAdmission {
    current: Option<(CacheScope, SessionRevision, PublicSceneKind)>,
}
impl SceneAdmission {
    pub(crate) fn new() -> Self {
        Self { current: None }
    }
    pub(crate) fn check(
        &self,
        scope: CacheScope,
        revision: SessionRevision,
        asset: PublicSceneAsset,
    ) -> Result<Admission, PublicSceneError> {
        match self.current {
            None => Ok(Admission::New),
            Some((old_scope, old_revision, kind)) if old_scope == scope => {
                if revision < old_revision {
                    return Err(PublicSceneError::StaleRevision);
                }
                if revision == old_revision {
                    return if kind == asset.kind {
                        Ok(Admission::Retained)
                    } else {
                        Err(PublicSceneError::ConflictingSnapshot)
                    };
                }
                if revision.epoch() == old_revision.epoch() && kind == asset.kind {
                    Ok(Admission::New)
                } else {
                    Ok(Admission::Replaced)
                }
            }
            Some(_) => Ok(Admission::Replaced),
        }
    }
    pub(crate) fn accept(
        &mut self,
        scope: CacheScope,
        revision: SessionRevision,
        asset: PublicSceneAsset,
    ) {
        self.current = Some((scope, revision, asset.kind));
    }
    pub(crate) fn clear(&mut self) {
        self.current = None;
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) mod browser {
    use super::*;
    use df_client::cache::{CacheLimits, FetchToken};
    use df_ui::{CampaignIllustration, CampaignSceneAssets, ConceptScene, SceneImageLimits};
    use futures::{
        FutureExt,
        future::{Either, select},
    };
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::{JsFuture, spawn_local};
    use web_sys::{
        AbortController, Document, Element, ReadableStreamDefaultReader, Request, RequestCache,
        RequestCredentials, RequestInit, RequestMode, Response,
    };

    const FETCH_DEADLINE_MS: u32 = 10_000;
    fn image_limits() -> SceneImageLimits {
        SceneImageLimits {
            cache: CacheLimits {
                max_assets: 2,
                max_pending: 1,
                max_leases: 1,
                max_bytes: HARBOR.byte_len as usize,
            },
            max_width: 1672,
            max_height: 941,
            max_pixels: 1_573_352,
        }
    }
    struct PendingFetch {
        identity: Rc<()>,
        controller: AbortController,
        cancelled: Rc<Cell<bool>>,
    }
    struct QueuedFetch {
        token: FetchToken,
        asset: PublicSceneAsset,
        selection: Rc<()>,
    }
    #[derive(Clone, Copy)]
    struct DeferredSelection {
        scope: CacheScope,
        revision: SessionRevision,
        asset: PublicSceneAsset,
    }
    struct State {
        admission: SceneAdmission,
        scope: Option<CacheScope>,
        selection: Rc<()>,
        active: Option<PendingFetch>,
        queued: Option<QueuedFetch>,
        deferred: Option<DeferredSelection>,
        reset: bool,
        failed: bool,
        closed: bool,
        failure: Option<PublicSceneError>,
    }
    pub(crate) struct PublicSceneDelivery {
        surface: CampaignIllustration,
        state: RefCell<State>,
    }
    impl PublicSceneDelivery {
        pub(crate) fn new(
            document: &Document,
            parent: &Element,
        ) -> Result<Rc<Self>, PublicSceneError> {
            let fallback = document
                .create_element("img")
                .map_err(|_| PublicSceneError::Browser)?;
            fallback.set_class_name("art");
            fallback
                .set_attribute("alt", "Authored scene concept illustration")
                .map_err(|_| PublicSceneError::Browser)?;
            parent
                .append_child(&fallback)
                .map_err(|_| PublicSceneError::Browser)?;
            let surface = match CampaignIllustration::mount_existing(parent, &fallback) {
                Ok(surface) => surface,
                Err(error) => {
                    fallback.remove();
                    return Err(PublicSceneError::Campaign(error));
                }
            };
            Ok(Rc::new(Self {
                surface,
                state: RefCell::new(State {
                    admission: SceneAdmission::new(),
                    scope: None,
                    selection: Rc::new(()),
                    active: None,
                    queued: None,
                    deferred: None,
                    reset: true,
                    failed: false,
                    closed: false,
                    failure: None,
                }),
            }))
        }
        pub(crate) fn is_attached(&self) -> bool {
            self.surface.is_attached()
        }
        pub(crate) fn attach(&self) -> Result<(), PublicSceneError> {
            self.surface.attach().map_err(PublicSceneError::Campaign)
        }
        pub(crate) fn take_failure(&self) -> Option<PublicSceneError> {
            self.state.borrow_mut().failure.take().or_else(|| {
                self.surface
                    .take_scene_failure()
                    .err()
                    .map(PublicSceneError::Campaign)
            })
        }
        /// Caller is the already admitted generated-view owner. Equal snapshots redraw
        /// current presentation but never manufacture a revision or restart failed work.
        pub(crate) fn update(
            self: &Rc<Self>,
            scope: CacheScope,
            revision: SessionRevision,
            path: &str,
        ) -> Result<(), PublicSceneError> {
            let asset = resolve(path)?;
            let mut state = self.state.borrow_mut();
            if state.closed {
                return Err(PublicSceneError::Closed);
            }
            let admission = state.admission.check(scope, revision, asset)?;
            if admission == Admission::Retained && (state.failed || !state.reset) {
                return Ok(());
            }
            if admission == Admission::Replaced {
                state.selection = Rc::new(());
                state.queued = None;
                if let Some(active) = &state.active {
                    active.cancelled.set(true);
                    active.controller.abort();
                }
            }
            if state.scope != Some(scope) || state.reset {
                // The campaign owner refuses a fresh image lifecycle while cancelled
                // real codec work drains. Preserve that owner and its finite reservation.
                if let Err(error) = self.surface.enable_scene_assets(scope, image_limits()) {
                    // Fresh accepted public fallback remains available while the old
                    // closed canonical codec keeps its original reservation until terminal.
                    self.surface
                        .set_concept_fallback(match asset.kind {
                            PublicSceneKind::Harbor => ConceptScene::MaraHarbor,
                            PublicSceneKind::Tavern => ConceptScene::Tavern,
                        })
                        .map_err(PublicSceneError::Campaign)?;
                    let weak = Rc::downgrade(self);
                    let deferred = self.surface.after_decode_terminal(Box::new(move || {
                        let Some(owner) = weak.upgrade() else {
                            return;
                        };
                        let pending = owner.state.borrow_mut().deferred.take();
                        if let Some(pending) = pending {
                            let path = pending
                                .asset
                                .path
                                .strip_prefix('/')
                                .unwrap_or(pending.asset.path);
                            if let Err(error) = owner.update(pending.scope, pending.revision, path)
                            {
                                owner.state.borrow_mut().failure = Some(error);
                            }
                        }
                    }));
                    if deferred {
                        state.queued = None;
                        state.deferred = Some(DeferredSelection {
                            scope,
                            revision,
                            asset,
                        });
                        state.admission.accept(scope, revision, asset);
                        state.reset = true;
                        state.failed = false;
                        return Ok(());
                    }
                    return Err(PublicSceneError::Campaign(error));
                }
                state.scope = Some(scope);
                state.reset = false;
                state.deferred = None;
            }
            state.failed = false;
            state.failure = None;
            let key = asset.key()?;
            let token = self
                .surface
                .update_with_scene_assets(
                    match asset.kind {
                        PublicSceneKind::Harbor => ConceptScene::MaraHarbor,
                        PublicSceneKind::Tavern => ConceptScene::Tavern,
                    },
                    CampaignSceneAssets {
                        scope,
                        revision,
                        references: std::slice::from_ref(&key),
                        selected: Some(&key),
                    },
                )
                .map_err(PublicSceneError::Campaign)?;
            state.admission.accept(scope, revision, asset);
            if let Some(token) = token {
                let selection = state.selection.clone();
                state.queued = Some(QueuedFetch {
                    token,
                    asset,
                    selection,
                });
            }
            drop(state);
            Self::start_queued(self)
        }
        fn start_queued(owner: &Rc<Self>) -> Result<(), PublicSceneError> {
            let mut state = owner.state.borrow_mut();
            if state.closed || state.active.is_some() {
                return Ok(());
            }
            let Some(queued) = state.queued.take() else {
                return Ok(());
            };
            let controller = match AbortController::new() {
                Ok(controller) => controller,
                Err(_) => {
                    state.failed = true;
                    state.reset = true;
                    owner
                        .surface
                        .show_fallback()
                        .map_err(PublicSceneError::Campaign)?;
                    return Err(PublicSceneError::Browser);
                }
            };
            let cancelled = Rc::new(Cell::new(false));
            let identity = Rc::new(());
            state.active = Some(PendingFetch {
                identity: identity.clone(),
                controller: controller.clone(),
                cancelled: cancelled.clone(),
            });
            let weak = Rc::downgrade(owner);
            drop(state);
            spawn_local(async move {
                let read =
                    read_exact(queued.asset, controller.clone(), cancelled.clone()).boxed_local();
                let timer =
                    gloo_timers::future::TimeoutFuture::new(FETCH_DEADLINE_MS).boxed_local();
                let result = match select(read, timer).await {
                    Either::Left((result, _)) => result,
                    Either::Right((_, remaining)) => {
                        cancelled.set(true);
                        controller.abort();
                        // The actual request/reader stays owned until its genuine terminal.
                        let _terminal = remaining.await;
                        Err(PublicSceneError::Timeout)
                    }
                };
                let Some(owner) = weak.upgrade() else {
                    return;
                };
                let mut state = owner.state.borrow_mut();
                if !state
                    .active
                    .as_ref()
                    .is_some_and(|active| Rc::ptr_eq(&active.identity, &identity))
                {
                    return;
                }
                state.active = None;
                if state.closed {
                    return;
                }
                let current = Rc::ptr_eq(&state.selection, &queued.selection);
                if current {
                    let completion = result.and_then(|bytes| {
                        owner
                            .surface
                            .complete_scene_asset(&queued.token, bytes)
                            .map_err(PublicSceneError::Campaign)
                    });
                    if let Err(error) = completion {
                        state.failure = Some(error);
                        state.failed = true;
                        state.reset = true;
                        // Retire the canonical fetch without touching caller text or controls.
                        if let Err(error) = owner.surface.show_fallback() {
                            state.failure = Some(PublicSceneError::Campaign(error));
                        }
                    }
                }
                drop(state);
                if let Err(error) = Self::start_queued(&owner) {
                    owner.state.borrow_mut().failure = Some(error);
                }
            });
            Ok(())
        }
        pub(crate) fn suspend(&self) -> Result<(), PublicSceneError> {
            let mut state = self.state.borrow_mut();
            if state.closed {
                return Err(PublicSceneError::Closed);
            }
            if state.active.is_none() && state.queued.is_none() && state.deferred.is_none() {
                return Ok(());
            }
            state.selection = Rc::new(());
            state.queued = None;
            state.deferred = None;
            state.reset = true;
            if let Some(active) = &state.active {
                active.cancelled.set(true);
                active.controller.abort();
            }
            self.surface
                .show_fallback()
                .map_err(PublicSceneError::Campaign)?;
            Ok(())
        }
        pub(crate) fn retire(&self) -> Result<(), PublicSceneError> {
            self.suspend()?;
            let mut state = self.state.borrow_mut();
            state.selection = Rc::new(());
            state.queued = None;
            state.deferred = None;
            state.reset = true;
            state.failed = false;
            state.admission.clear();
            self.surface.retire().map_err(PublicSceneError::Campaign)
        }
        pub(crate) fn dispose(&self) -> Result<(), PublicSceneError> {
            let mut state = self.state.borrow_mut();
            if state.closed {
                return Ok(());
            }
            state.closed = true;
            state.selection = Rc::new(());
            state.queued = None;
            state.deferred = None;
            state.admission.clear();
            if let Some(active) = &state.active {
                active.cancelled.set(true);
                active.controller.abort();
            }
            self.surface.dispose().map_err(PublicSceneError::Campaign)
        }
        pub(crate) fn work_counts(&self) -> (usize, usize) {
            let state = self.state.borrow();
            (
                usize::from(state.active.is_some()),
                usize::from(state.queued.is_some() || state.deferred.is_some()),
            )
        }
    }
    impl Drop for PublicSceneDelivery {
        fn drop(&mut self) {
            let _cleanup = self.dispose();
        }
    }

    async fn read_exact(
        asset: PublicSceneAsset,
        controller: AbortController,
        cancelled: Rc<Cell<bool>>,
    ) -> Result<Vec<u8>, PublicSceneError> {
        let window = web_sys::window().ok_or(PublicSceneError::Browser)?;
        let options = RequestInit::new();
        options.set_method("GET");
        options.set_mode(RequestMode::SameOrigin);
        options.set_credentials(RequestCredentials::Omit);
        options.set_cache(RequestCache::NoStore);
        options.set_signal(Some(&controller.signal()));
        let request = Request::new_with_str_and_init(asset.path, &options)
            .map_err(|_| PublicSceneError::Browser)?;
        let response = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|_| {
                if cancelled.get() {
                    PublicSceneError::Cancelled
                } else {
                    PublicSceneError::Transport
                }
            })?
            .dyn_into::<Response>()
            .map_err(|_| PublicSceneError::Browser)?;
        // Header refusal owns cancellation until the response stream acknowledges it.
        let admitted = (|| {
            if cancelled.get() {
                return Err(PublicSceneError::Cancelled);
            }
            if !response.ok() {
                return Err(PublicSceneError::HttpStatus(response.status()));
            }
            let length = response
                .headers()
                .get("content-length")
                .map_err(|_| PublicSceneError::Browser)?
                .ok_or(PublicSceneError::MissingLength)?
                .parse::<u64>()
                .map_err(|_| PublicSceneError::LengthMismatch)?;
            if length != asset.byte_len {
                return Err(PublicSceneError::LengthMismatch);
            }
            Ok(())
        })();
        if let Err(error) = admitted {
            controller.abort();
            if let Some(body) = response.body() {
                let _terminal = JsFuture::from(body.cancel()).await;
            }
            return Err(error);
        }
        let body = response.body().ok_or(PublicSceneError::Transport)?;
        let reader = match ReadableStreamDefaultReader::new(&body) {
            Ok(reader) => reader,
            Err(_) => {
                controller.abort();
                let _terminal = JsFuture::from(body.cancel()).await;
                return Err(PublicSceneError::Browser);
            }
        };
        let result = read_body(&reader, asset.byte_len, &cancelled).await;
        if result.is_err() {
            controller.abort();
            // Resolve or reject is a terminal acknowledgement; neither is a byte success.
            let _terminal = JsFuture::from(reader.cancel()).await;
        }
        reader.release_lock();
        result
    }
    async fn read_body(
        reader: &ReadableStreamDefaultReader,
        byte_len: u64,
        cancelled: &Cell<bool>,
    ) -> Result<Vec<u8>, PublicSceneError> {
        let capacity = usize::try_from(byte_len).map_err(|_| PublicSceneError::Oversized)?;
        let mut bytes = Vec::with_capacity(capacity);
        loop {
            let result = JsFuture::from(reader.read()).await.map_err(|_| {
                if cancelled.get() {
                    PublicSceneError::Cancelled
                } else {
                    PublicSceneError::Transport
                }
            })?;
            if cancelled.get() {
                return Err(PublicSceneError::Cancelled);
            }
            let done = js_sys::Reflect::get(&result, &JsValue::from_str("done"))
                .map_err(|_| PublicSceneError::Browser)?
                .as_bool()
                .ok_or(PublicSceneError::Browser)?;
            if done {
                break;
            }
            let chunk = js_sys::Reflect::get(&result, &JsValue::from_str("value"))
                .map_err(|_| PublicSceneError::Browser)?
                .dyn_into::<js_sys::Uint8Array>()
                .map_err(|_| PublicSceneError::Browser)?;
            let previous = bytes.len();
            let length = previous
                .checked_add(chunk.length() as usize)
                .ok_or(PublicSceneError::Oversized)?;
            if length > capacity {
                return Err(PublicSceneError::Oversized);
            }
            bytes.resize(length, 0);
            chunk.copy_to(
                bytes
                    .get_mut(previous..length)
                    .ok_or(PublicSceneError::Oversized)?,
            );
        }
        if bytes.len() != capacity {
            return Err(PublicSceneError::LengthMismatch);
        }
        Ok(bytes)
    }
}
