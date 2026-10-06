use crate::{
    DecodeBudget, DecodeToken, FlatScene, PresentationOutcome, RenderError, RendererCapabilities,
    RendererResource, ResourceCache, ResourceError, ResourceLimits, ResourceReadiness, SceneOwner,
    SceneRenderer, WorkOutcome,
};
use df_types::RevisionLabel;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceSceneError {
    Scene(RenderError),
    Resource(ResourceError),
    Cleanup {
        original: RenderError,
        cleanup: RenderError,
    },
}

/// A single scene and its decoded resources; callers retain this owner until all pending
/// work reaches a terminal callback. Disposed owners cannot publish late surfaces.
pub struct ResourceSceneRenderer<R: RendererResource> {
    scene: SceneRenderer,
    resources: ResourceCache<R>,
}

impl<R: RendererResource> ResourceSceneRenderer<R> {
    pub fn from_scene(
        scene: SceneRenderer,
        owner: SceneOwner,
        limits: ResourceLimits,
        preparation_revision: RevisionLabel,
    ) -> Result<Self, ResourceError> {
        // Only a newly constructed owner can be wrapped: an already accepted watermark
        // would otherwise let duplicate snapshots skip initial cache reference admission.
        if scene.current().is_some() {
            return Err(ResourceError::SceneAlreadyInitialized);
        }
        let resources = ResourceCache::new(owner, limits, preparation_revision)?;
        Ok(Self { scene, resources })
    }

    #[cfg(target_arch = "wasm32")]
    pub fn mount(
        parent: &web_sys::Element,
        owner: SceneOwner,
        limits: ResourceLimits,
        preparation_revision: RevisionLabel,
    ) -> Result<Self, ResourceSceneError> {
        let scene = SceneRenderer::mount(parent, owner).map_err(ResourceSceneError::Scene)?;
        Self::from_scene(scene, owner, limits, preparation_revision)
            .map_err(ResourceSceneError::Resource)
    }

    pub(crate) fn check_update(
        &self,
        owner: SceneOwner,
        flat: &FlatScene,
        references: &[R::Key],
    ) -> Result<Option<PresentationOutcome>, ResourceSceneError> {
        if let Some(outcome) = self
            .scene
            .check_update(owner, flat)
            .map_err(ResourceSceneError::Scene)?
        {
            return Ok(Some(outcome));
        }
        self.resources
            .validate_scene(owner, flat.revision, references)
            .map_err(ResourceSceneError::Resource)?;
        Ok(None)
    }

    /// Geometry/reference validation is atomic. Stale/duplicate snapshots preserve both.
    /// Browser mutation failure closes resources and the scene, preserving cleanup errors.
    pub fn update(
        &mut self,
        owner: SceneOwner,
        scene_label: RevisionLabel,
        flat: FlatScene,
        references: &[R::Key],
    ) -> Result<PresentationOutcome, ResourceSceneError> {
        if let Some(outcome) = self.check_update(owner, &flat, references)? {
            return Ok(outcome);
        }
        let revision = flat.revision;
        let outcome = match self.scene.update(owner, flat) {
            Ok(outcome) => outcome,
            Err(original) => {
                self.resources.dispose();
                return match self.scene.dispose() {
                    Ok(_) => Err(ResourceSceneError::Scene(original)),
                    Err(cleanup) => Err(ResourceSceneError::Cleanup { original, cleanup }),
                };
            }
        };
        // The synchronous owner cannot change between preflight and application.
        self.resources
            .apply_scene(owner, revision, scene_label, references)
            .map_err(ResourceSceneError::Resource)?;
        Ok(outcome)
    }

    /// Returns the canonical owner scope; this grants no source authorization.
    pub fn owner(&self) -> SceneOwner {
        self.scene.owner()
    }

    pub fn scene(&self) -> &SceneRenderer {
        &self.scene
    }
    pub fn current(&self) -> Option<&FlatScene> {
        self.scene.current()
    }
    pub fn capabilities(&self) -> RendererCapabilities {
        self.scene.capabilities()
    }
    pub fn begin(
        &mut self,
        key: &R::Key,
        budget: DecodeBudget,
        abort: impl FnOnce() + 'static,
    ) -> Result<DecodeToken<R::Key>, ResourceError> {
        self.resources.begin(key, budget, abort)
    }
    pub fn complete(
        &mut self,
        token: &DecodeToken<R::Key>,
        resource: R,
    ) -> Result<(), ResourceError> {
        self.resources.complete(token, resource)
    }
    pub fn finish_failed(
        &mut self,
        token: &DecodeToken<R::Key>,
    ) -> Result<WorkOutcome, ResourceError> {
        self.resources.finish_failed(token)
    }
    pub fn cancel(&mut self, token: &DecodeToken<R::Key>) -> Result<(), ResourceError> {
        self.resources.cancel(token)
    }
    pub fn release(&mut self, key: &R::Key) -> Result<(), ResourceError> {
        self.resources.release(key)
    }
    pub fn get(&mut self, key: &R::Key) -> Result<Option<&R>, ResourceError> {
        self.resources.get(key)
    }
    pub fn readiness(&self, key: &R::Key) -> Result<ResourceReadiness, ResourceError> {
        self.resources.readiness(key)
    }
    pub fn decoded_bytes(&self) -> usize {
        self.resources.decoded_bytes()
    }
    pub fn work_bytes(&self) -> usize {
        self.resources.work_bytes()
    }
    pub fn pending_count(&self) -> usize {
        self.resources.pending_count()
    }
    pub fn preparation_revision(&self) -> &RevisionLabel {
        self.resources.preparation_revision()
    }
    pub fn dispose(&mut self) -> Result<PresentationOutcome, RenderError> {
        self.resources.dispose();
        self.scene.dispose()
    }
}

impl<R: RendererResource> Drop for ResourceSceneRenderer<R> {
    fn drop(&mut self) {
        // SceneRenderer owns its own terminal Drop cleanup. Here no fallible browser
        // operation is repeated or discarded: only infallible resource retirement occurs.
        self.resources.dispose();
    }
}
