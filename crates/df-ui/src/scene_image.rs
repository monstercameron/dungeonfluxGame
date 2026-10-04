use df_client::cache::{CacheError, CacheKey, CacheLimits, CacheScope};
use df_types::SessionRevision;

/// Complete already permitted caller references; identities are correlation, not access.
#[derive(Clone, Copy)]
pub struct CampaignSceneAssets<'a> {
    pub scope: CacheScope,
    pub revision: SessionRevision,
    pub references: &'a [CacheKey],
    pub selected: Option<&'a CacheKey>,
}

/// Finite encoded and decoded image bounds selected by the mounted owner.
#[derive(Clone, Copy, Debug)]
pub struct SceneImageLimits {
    pub cache: CacheLimits,
    pub max_width: u32,
    pub max_height: u32,
    pub max_pixels: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SceneImageError {
    InvalidLimits,
    NotEnabled,
    NotSelected,
    InvalidPng,
    Dimensions,
    Cache(CacheError),
}

impl SceneImageLimits {
    pub fn validate(self) -> Result<(), SceneImageError> {
        if self.max_width == 0
            || self.max_height == 0
            || self.max_pixels == 0
            || self.cache.max_assets == 0
            || self.cache.max_pending == 0
            || self.cache.max_leases == 0
            || self.cache.max_bytes == 0
        {
            return Err(SceneImageError::InvalidLimits);
        }
        Ok(())
    }

    #[cfg(any(target_arch = "wasm32", test))]
    fn dimensions(self, bytes: &[u8]) -> Result<(u32, u32), SceneImageError> {
        // IHDR is fixed size. Validate dimensions before any browser image decoder sees bytes.
        if bytes.len() < 33
            || bytes.get(..8) != Some(b"\x89PNG\r\n\x1a\n".as_slice())
            || bytes.get(8..12) != Some([0, 0, 0, 13].as_slice())
            || bytes.get(12..16) != Some(b"IHDR".as_slice())
        {
            return Err(SceneImageError::InvalidPng);
        }
        let width = u32::from_be_bytes(
            bytes
                .get(16..20)
                .and_then(|v| v.try_into().ok())
                .ok_or(SceneImageError::InvalidPng)?,
        );
        let height = u32::from_be_bytes(
            bytes
                .get(20..24)
                .and_then(|v| v.try_into().ok())
                .ok_or(SceneImageError::InvalidPng)?,
        );
        if width == 0
            || height == 0
            || width > self.max_width
            || height > self.max_height
            || u64::from(width) * u64::from(height) > self.max_pixels
        {
            return Err(SceneImageError::Dimensions);
        }
        let valid_depth = matches!(
            (bytes.get(25), bytes.get(24)),
            (Some(0), Some(1 | 2 | 4 | 8 | 16))
                | (Some(2 | 4 | 6), Some(8 | 16))
                | (Some(3), Some(1 | 2 | 4 | 8))
        );
        if !valid_depth
            || bytes.get(26) != Some(&0)
            || bytes.get(27) != Some(&0)
            || !matches!(bytes.get(28), Some(0 | 1))
        {
            return Err(SceneImageError::InvalidPng);
        }
        Ok((width, height))
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) mod browser {
    use super::*;
    use crate::CampaignError;
    use df_client::cache::{AssetCache, CacheLease, FetchToken};
    use std::{
        cell::RefCell,
        rc::{Rc, Weak},
    };
    use wasm_bindgen::{JsCast, closure::Closure};
    use web_sys::{Blob, BlobPropertyBag, Element, Event, HtmlImageElement, Url};

    struct Listener {
        node: HtmlImageElement,
        event: &'static str,
        callback: Closure<dyn FnMut(Event)>,
        registered: bool,
    }
    impl Listener {
        fn new(
            node: &HtmlImageElement,
            event: &'static str,
            callback: impl FnMut(Event) + 'static,
        ) -> Result<Self, CampaignError> {
            let callback = Closure::wrap(Box::new(callback) as Box<dyn FnMut(Event)>);
            node.add_event_listener_with_callback(event, callback.as_ref().unchecked_ref())?;
            Ok(Self {
                node: node.clone(),
                event,
                callback,
                registered: true,
            })
        }
        fn unregister(&mut self) -> Result<(), CampaignError> {
            if self.registered {
                self.node.remove_event_listener_with_callback(
                    self.event,
                    self.callback.as_ref().unchecked_ref(),
                )?;
                self.registered = false;
            }
            Ok(())
        }
    }
    impl Drop for Listener {
        fn drop(&mut self) {
            // The owner explicitly reports cleanup errors; Drop still fences this callback.
            let _cleanup = self.unregister();
        }
    }
    struct Slot {
        identity: Rc<()>,
        key: CacheKey,
        lease: Option<CacheLease>,
        image: HtmlImageElement,
        url: Option<String>,
        listeners: Vec<Listener>,
        displayed: bool,
        dimensions: (u32, u32),
    }

    /// The only owner of encoded bytes, one browser PNG slot and its callback identities.
    pub(crate) struct SceneImageOwner {
        cache: AssetCache,
        scope: CacheScope,
        limits: SceneImageLimits,
        revision: Option<SessionRevision>,
        selected: Option<CacheKey>,
        requested: Option<FetchToken>,
        root: Element,
        fallback: Element,
        slot: Option<Slot>,
        closed: bool,
        failure: Option<CampaignError>,
    }
    impl SceneImageOwner {
        pub(crate) fn new(
            scope: CacheScope,
            limits: SceneImageLimits,
            root: &Element,
            fallback: &Element,
        ) -> Result<Rc<RefCell<Self>>, CampaignError> {
            limits.validate().map_err(CampaignError::SceneImage)?;
            let cache = AssetCache::new(scope, limits.cache)
                .map_err(|e| CampaignError::SceneImage(SceneImageError::Cache(e)))?;
            Ok(Rc::new(RefCell::new(Self {
                cache,
                scope,
                limits,
                revision: None,
                selected: None,
                requested: None,
                root: root.clone(),
                fallback: fallback.clone(),
                slot: None,
                closed: false,
                failure: None,
            })))
        }
        pub(crate) fn take_failure(&mut self) -> Result<(), CampaignError> {
            match self.failure.take() {
                Some(error) => Err(error),
                None => Ok(()),
            }
        }
        pub(crate) fn validate(
            &self,
            assets: CampaignSceneAssets<'_>,
        ) -> Result<(), CampaignError> {
            let error = if self.closed {
                Some(SceneImageError::Cache(CacheError::Closed))
            } else if self.scope != assets.scope {
                Some(SceneImageError::Cache(CacheError::WrongScope))
            } else if self
                .revision
                .is_some_and(|current| assets.revision <= current)
            {
                Some(SceneImageError::Cache(CacheError::StaleRevision))
            } else if assets.references.len() > self.limits.cache.max_assets {
                Some(SceneImageError::Cache(CacheError::ReferenceCapacity))
            } else if assets
                .selected
                .is_some_and(|key| !assets.references.contains(key))
            {
                Some(SceneImageError::NotSelected)
            } else {
                None
            };
            if let Some(error) = error {
                return Err(CampaignError::SceneImage(error));
            }
            for (index, key) in assets.references.iter().enumerate() {
                if assets
                    .references
                    .iter()
                    .take(index)
                    .any(|old| old.version == key.version)
                {
                    return Err(CampaignError::SceneImage(SceneImageError::Cache(
                        CacheError::ConflictingReference,
                    )));
                }
            }
            Ok(())
        }
        pub(crate) fn reconcile(
            owner: &Rc<RefCell<Self>>,
            assets: CampaignSceneAssets<'_>,
        ) -> Result<Option<FetchToken>, CampaignError> {
            let mut current = owner.borrow_mut();
            current.validate(assets)?;
            let recovery = current
                .revision
                .is_some_and(|old| old.epoch() != assets.revision.epoch());
            current
                .cache
                .apply_current(assets.scope, assets.revision, assets.references)
                .map_err(|e| CampaignError::SceneImage(SceneImageError::Cache(e)))?;
            current.revision = Some(assets.revision);
            let replaced = recovery || current.selected.as_ref() != assets.selected;
            current.selected = assets.selected.cloned();
            if replaced {
                if let Some(request) = current.requested.take() {
                    match current.cache.cancel(&request) {
                        Ok(()) | Err(CacheError::StaleFetch) => {}
                        Err(e) => return Err(CampaignError::SceneImage(SceneImageError::Cache(e))),
                    }
                }
                current.clear_slot()?;
            }
            let Some(key) = current.selected.clone() else {
                return Ok(None);
            };
            if current.slot.as_ref().is_some_and(|slot| {
                slot.key == key
                    && slot
                        .lease
                        .as_ref()
                        .is_some_and(|lease| current.cache.lease_bytes(lease).is_ok())
            }) {
                return Ok(None);
            }
            if current.requested.is_some() {
                return Ok(None);
            }
            if current
                .cache
                .get(&key)
                .map_err(|e| CampaignError::SceneImage(SceneImageError::Cache(e)))?
                .is_some()
            {
                drop(current);
                Self::install(owner, &key)?;
                return Ok(None);
            }
            let token = current
                .cache
                .fetch(&key)
                .map_err(|e| CampaignError::SceneImage(SceneImageError::Cache(e)))?;
            current.requested = Some(token.clone());
            Ok(Some(token))
        }
        pub(crate) fn complete(
            owner: &Rc<RefCell<Self>>,
            token: &FetchToken,
            bytes: Vec<u8>,
        ) -> Result<(), CampaignError> {
            let mut current = owner.borrow_mut();
            if current.closed || current.selected.as_ref() != Some(token.key()) {
                return Err(CampaignError::SceneImage(SceneImageError::NotSelected));
            }
            if let Err(error) = current.cache.complete(token, bytes) {
                if !matches!(error, CacheError::StaleFetch | CacheError::Closed) {
                    current.requested = None;
                }
                return Err(CampaignError::SceneImage(SceneImageError::Cache(error)));
            }
            current.requested = None;
            drop(current);
            Self::install(owner, token.key())
        }
        fn install(owner: &Rc<RefCell<Self>>, key: &CacheKey) -> Result<(), CampaignError> {
            let mut current = owner.borrow_mut();
            let lease = current
                .cache
                .acquire(key)
                .map_err(|e| CampaignError::SceneImage(SceneImageError::Cache(e)))?
                .ok_or(CampaignError::SceneImage(SceneImageError::NotSelected))?;
            let bytes = current
                .cache
                .lease_bytes(&lease)
                .map_err(|e| CampaignError::SceneImage(SceneImageError::Cache(e)))?;
            let dimensions = match current.limits.dimensions(bytes) {
                Ok(dimensions) => dimensions,
                Err(error) => {
                    current
                        .cache
                        .release(key)
                        .map_err(|e| CampaignError::SceneImage(SceneImageError::Cache(e)))?;
                    return Err(CampaignError::SceneImage(error));
                }
            };
            let array = js_sys::Uint8Array::from(bytes);
            let parts = js_sys::Array::new();
            parts.push(&array);
            let options = BlobPropertyBag::new();
            options.set_type("image/png");
            let blob = Blob::new_with_u8_array_sequence_and_options(&parts, &options)?;
            let image: HtmlImageElement = current
                .root
                .owner_document()
                .ok_or(CampaignError::Disposed)?
                .create_element("img")?
                .dyn_into()
                .map_err(|_| CampaignError::SceneImage(SceneImageError::InvalidPng))?;
            image.set_class_name("scene-art");
            image.set_alt(
                current
                    .fallback
                    .get_attribute("alt")
                    .as_deref()
                    .unwrap_or("Scene artwork"),
            );
            let identity = Rc::new(());
            current.clear_slot()?;
            let url = Url::create_object_url_with_blob(&blob)?;
            current.slot = Some(Slot {
                identity: identity.clone(),
                key: key.clone(),
                lease: Some(lease),
                image: image.clone(),
                url: Some(url.clone()),
                listeners: Vec::new(),
                displayed: false,
                dimensions,
            });
            drop(current);
            let mut listeners = Vec::new();
            for (event, success) in [("load", true), ("error", false)] {
                let weak: Weak<RefCell<Self>> = Rc::downgrade(owner);
                let operation = identity.clone();
                let listener = Listener::new(&image, event, move |_| {
                    if let Some(owner) = weak.upgrade() {
                        let mut owner = owner.borrow_mut();
                        if let Err(error) = owner.loaded(&operation, success) {
                            // The next owning API reports the typed DOM failure. Input remains usable.
                            owner.failure = Some(error);
                            if let Err(cleanup) = owner.retire_slot() {
                                owner.failure = Some(cleanup);
                            }
                        }
                    }
                });
                match listener {
                    Ok(listener) => listeners.push(listener),
                    Err(error) => {
                        owner.borrow_mut().clear_slot()?;
                        return Err(error);
                    }
                }
            }
            owner
                .borrow_mut()
                .slot
                .as_mut()
                .ok_or(CampaignError::Disposed)?
                .listeners = listeners;
            let loading = owner
                .borrow()
                .root
                .set_attribute("data-scene-image", "loading");
            if let Err(error) = loading {
                owner.borrow_mut().clear_slot()?;
                return Err(error.into());
            }
            image.set_src(&url);
            Ok(())
        }
        fn loaded(&mut self, identity: &Rc<()>, success: bool) -> Result<(), CampaignError> {
            let Some(slot) = &self.slot else {
                return Ok(());
            };
            if self.closed || !Rc::ptr_eq(&slot.identity, identity) {
                return Ok(());
            }
            let valid = (slot.image.natural_width(), slot.image.natural_height())
                == slot.dimensions
                && self.selected.as_ref() == Some(&slot.key)
                && slot
                    .lease
                    .as_ref()
                    .is_some_and(|lease| self.cache.lease_bytes(lease).is_ok());
            if !valid || !success {
                // Unregister now, but retain executing closures until the next owner operation.
                self.retire_slot()?;
                return Ok(());
            }
            if !slot.displayed {
                let parent = self.fallback.parent_node().ok_or(CampaignError::Disposed)?;
                parent.replace_child(&slot.image, &self.fallback)?;
                if let Some(slot) = self.slot.as_mut() {
                    slot.displayed = true;
                }
            }
            self.root.set_attribute("data-scene-image", "generated")?;
            Self::failed_class(&self.root, false);
            Ok(())
        }
        fn failed_class(root: &Element, failed: bool) {
            let mut names: Vec<_> = root
                .class_name()
                .split_whitespace()
                .filter(|name| *name != "exploration-art-failed")
                .map(str::to_owned)
                .collect();
            if failed {
                names.push("exploration-art-failed".to_owned());
            }
            root.set_class_name(&names.join(" "));
        }
        fn retire_slot(&mut self) -> Result<(), CampaignError> {
            let mut failure = None;
            if let Some(slot) = self.slot.as_mut() {
                if slot.displayed {
                    if let Some(parent) = slot.image.parent_node()
                        && let Err(e) = parent.replace_child(&self.fallback, &slot.image)
                    {
                        failure = Some(CampaignError::from(e));
                    }
                    slot.displayed = false;
                }
                slot.image.remove();
                slot.image
                    .remove_attribute("src")
                    .map_err(CampaignError::from)
                    .unwrap_or_else(|e| {
                        if failure.is_none() {
                            failure = Some(e);
                        }
                    });
                for listener in &mut slot.listeners {
                    if let Err(e) = listener.unregister()
                        && failure.is_none()
                    {
                        failure = Some(e);
                    }
                }
                if let Some(url) = slot.url.take()
                    && let Err(e) = Url::revoke_object_url(&url)
                    && failure.is_none()
                {
                    failure = Some(e.into());
                }
                if let Some(lease) = slot.lease.take() {
                    match self.cache.release_lease(&lease) {
                        Ok(()) | Err(CacheError::StaleLease | CacheError::Closed) => {}
                        Err(e) if failure.is_none() => {
                            failure = Some(CampaignError::SceneImage(SceneImageError::Cache(e)))
                        }
                        Err(_) => {}
                    }
                }
            }
            // The exact current concept image may have finished with an error while
            // detached behind the generated PNG. Reconcile its actual readiness now;
            // no old concept event or generated event can supply this state.
            let failed = self
                .fallback
                .dyn_ref::<HtmlImageElement>()
                .is_some_and(|image| {
                    image.has_attribute("src") && image.complete() && image.natural_width() == 0
                });
            Self::failed_class(&self.root, failed);
            self.root
                .set_attribute("data-scene-image", "fallback")
                .map_err(CampaignError::from)
                .unwrap_or_else(|e| {
                    if failure.is_none() {
                        failure = Some(e);
                    }
                });
            match failure {
                Some(error) => Err(error),
                None => Ok(()),
            }
        }
        fn clear_slot(&mut self) -> Result<(), CampaignError> {
            let result = self.retire_slot();
            self.slot = None;
            result
        }
        pub(crate) fn replace_fallback(&mut self, fallback: &Element) {
            self.fallback = fallback.clone();
        }
        pub(crate) fn set_description(&self, description: &str) {
            if let Some(slot) = &self.slot {
                slot.image.set_alt(description);
            }
        }
        pub(crate) fn legacy(&mut self) -> Result<(), CampaignError> {
            let cleanup = self.clear_slot();
            self.cache = AssetCache::new(self.scope, self.limits.cache)
                .map_err(|e| CampaignError::SceneImage(SceneImageError::Cache(e)))?;
            self.selected = None;
            self.requested = None;
            cleanup
        }
        pub(crate) fn dispose(&mut self) -> Result<(), CampaignError> {
            self.closed = true;
            let result = self.clear_slot();
            self.cache.dispose();
            self.selected = None;
            self.requested = None;
            result
        }
    }
    impl Drop for SceneImageOwner {
        fn drop(&mut self) {
            let _cleanup = self.dispose();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn limits() -> SceneImageLimits {
        SceneImageLimits {
            cache: CacheLimits {
                max_assets: 1,
                max_pending: 1,
                max_leases: 1,
                max_bytes: 64,
            },
            max_width: 8,
            max_height: 8,
            max_pixels: 32,
        }
    }
    fn header(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
        bytes
    }
    #[test]
    fn png_dimensions_are_checked_before_decode() {
        assert_eq!(limits().dimensions(&header(4, 4)), Ok((4, 4)));
        for (w, h) in [(0, 1), (1, 0), (9, 1), (8, 8), (u32::MAX, u32::MAX)] {
            assert_eq!(
                limits().dimensions(&header(w, h)),
                Err(SceneImageError::Dimensions)
            );
        }
    }
    #[test]
    fn truncated_non_png_and_unsupported_ihdr_are_refused() {
        let valid = header(1, 1);
        for size in 0..33 {
            assert_eq!(
                limits().dimensions(&valid[..size]),
                Err(SceneImageError::InvalidPng)
            );
        }
        let mut wrong = valid.clone();
        wrong[0] = 0;
        assert_eq!(
            limits().dimensions(&wrong),
            Err(SceneImageError::InvalidPng)
        );
        wrong = valid.clone();
        wrong[26] = 1;
        assert_eq!(
            limits().dimensions(&wrong),
            Err(SceneImageError::InvalidPng)
        );
    }
}
