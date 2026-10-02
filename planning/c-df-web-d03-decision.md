# Persistent route and resource disposal decision

Date: 2026-10-02  
Task: `B-C-df-web-D03-a1`  
Input: `d870db00dc349425e8f977bdfb20c6ffb24bfd3e`

## Decision

`df-web` owns one Rust/WASM shell for the page lifetime and one replaceable
mounted route scope within it. Normal role or panel navigation, view updates,
phase changes, and reconnect reconcile the mounted Rust composition in place;
they do not reload the page or recreate the shell. The route is presentation
selection only. It cannot establish identity, audience, membership, or host
permission. A protected panel mounts only after a separate authorized view or
permission supplied through the approved client boundary.

The active route scope owns every resource whose lifetime is bounded by that
mount: browser listener registrations, client subscriptions, pending asset or
decode work, frame callbacks/animation handles, and renderer/audio handles
registered with that scope. `df-web` owns scope replacement and terminal shell
disposal; each adapter remains responsible for the concrete cancellation or
release operation of the handle it returns. A scope replacement first marks the
old generation inactive, then cancels/releases its owned handles exactly once,
then installs the new scope. Shell disposal is terminal and idempotent. A failed
mount leaves no newly registered resource detached from an owner.

Every asynchronous completion carries its captured route generation and
resource key. The scope accepts publication only while it is still active and
both values match; cancellation alone is not proof that a queued completion has
gone away. Replacing or disposing a scope invalidates its generation before
release. Hidden-tab suspension and reconnect preserve the shell but may stop,
seek, skip, or cancel scope work according to the relevant presentation
contract; they do not revive obsolete handles or publish stale results. Resource
caches and pending work remain bounded under their owning adapter contracts.

This decision freezes lifecycle ownership and ordering only. It does not define
a framework-specific route registry, browser binding type, RPC message, task
executor, audio policy, or concrete resource-handle API. Browser bindings remain
generated glue; application and UI behavior remains Rust. `df-client` owns the
connection and ordered-view boundary, `df-player`/`df-display` own role
composition, `df-ui` owns reusable components, `df-render` owns scene
presentation, and `df-audio` owns playback/capture. `df-web` composes these
owners without taking over their internal resource operations.

## Bounded Rust contract example

This standalone example demonstrates the valid path and refusals: replacing a
route releases the old scope, stale completions are rejected, and terminal
disposal is idempotent. `ResourceLease` represents a bundle of concrete handles;
in production the scope must own the actual listener/subscription/task/decode/
animation/render/audio handles and invoke their adapter-defined release or
cancellation behavior. This example does not claim a production browser route
implementation or invent a browser/RPC API.

```rust
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Route {
    Player,
    SharedDisplay,
    CustomerService,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScopeToken {
    route: Route,
    generation: u64,
    resource_key: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RouteError {
    ShellDisposed,
    GenerationExhausted,
}

struct ResourceLease(Arc<AtomicUsize>);

impl Drop for ResourceLease {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

struct MountedScope {
    token: ScopeToken,
    _resources: ResourceLease,
}

struct Shell {
    generation: u64,
    mounted: Option<MountedScope>,
    disposed: bool,
}

impl Shell {
    fn new() -> Self {
        Self {
            generation: 0,
            mounted: None,
            disposed: false,
        }
    }

    fn navigate(
        &mut self,
        route: Route,
        resource_key: u64,
        resources: ResourceLease,
    ) -> Result<ScopeToken, RouteError> {
        if self.disposed {
            return Err(RouteError::ShellDisposed);
        }
        let generation = self
            .generation
            .checked_add(1)
            .ok_or(RouteError::GenerationExhausted)?;
        self.generation = generation;
        self.mounted = None;
        let token = ScopeToken {
            route,
            generation,
            resource_key,
        };
        self.mounted = Some(MountedScope {
            token,
            _resources: resources,
        });
        Ok(token)
    }

    fn accepts_completion(&self, token: ScopeToken) -> bool {
        !self.disposed
            && self
                .mounted
                .as_ref()
                .is_some_and(|scope| scope.token == token)
    }

    fn dispose(&mut self) {
        self.generation = self.generation.saturating_add(1);
        self.mounted = None;
        self.disposed = true;
    }
}

fn main() {
    let released = Arc::new(AtomicUsize::new(0));
    let mut shell = Shell::new();

    let player = shell
        .navigate(Route::Player, 101, ResourceLease(Arc::clone(&released)))
        .expect("active shell accepts a route");
    assert!(shell.accepts_completion(player));

    let display = shell
        .navigate(
            Route::SharedDisplay,
            202,
            ResourceLease(Arc::clone(&released)),
        )
        .expect("active shell accepts route replacement");
    assert_eq!(released.load(Ordering::SeqCst), 1);
    assert!(!shell.accepts_completion(player));
    assert!(shell.accepts_completion(display));
    assert!(!shell.accepts_completion(ScopeToken {
        resource_key: 999,
        ..display
    }));

    shell.dispose();
    shell.dispose();
    assert_eq!(released.load(Ordering::SeqCst), 2);
    assert!(!shell.accepts_completion(display));

    let refused = shell.navigate(
        Route::CustomerService,
        303,
        ResourceLease(Arc::clone(&released)),
    );
    assert_eq!(refused, Err(RouteError::ShellDisposed));
    assert_eq!(released.load(Ordering::SeqCst), 3);
    println!("PASS: navigation replaces and releases the active route scope");
    println!("PASS: stale completion and navigation after disposal are refused");
}
```

The route enum is a presentation target, not an authorization grant. The sample's
drop counter shows ownership release only; production behavior must be proved
against actual browser handles and the owning adapters. The next consumer is the
`df-web` route/composition owner, using the already planned
`df-client`/`df-player`/`df-display`/`df-ui`/`df-render`/`df-audio` boundaries.

## Alternatives and unresolved facts

- Reloading the page for navigation or reconnect loses valid local presentation
  state and makes scope cleanup dependent on browser teardown; it violates the
  persistent-shell contract.
- Keeping route resources at shell lifetime leaves old route listeners,
  subscriptions, tasks, and render/audio handles active after navigation.
- Relying on cancellation alone permits already queued work to publish into a
  replacement route; completion validation needs both the active generation and
  resource key.
- Framework, browser binding, route registry, resource handle representations,
  concrete cancellation APIs, and measured bounds remain at the G01/G02/G04/G08
  and consumer-interface gates. No `df-web` runtime exists in this input
  revision, so this decision and its example do not establish integrated route
  or browser behavior.
- The existing browser fixture is transport/test tooling, not an application
  shell or evidence that route-scoped handles are disposed.

## Evidence and handoff

This decision is grounded in `planning/subsystem-architecture.md` (Design,
Browser crates, Integration and refinement),
`planning/subsystem-interfaces.md` (Common contract rules, Browser interfaces),
`planning/client-architecture.md`, `planning/client-presentation.md` (Update and
resource lifecycle, Motion/time/interruption), `planning/runtime-reliability.md`
(cancellation, stale result identity), and
`planning/commerce-service.md` (Mounted Rust customer experience). These are
planning contracts at the frozen input revision; no runtime source or executable
check is claimed here.

The next implementation must be owned by the existing `df-web` startup/route
composition source with only the minimum approved consumer paths needed to
acquire and release actual adapter-owned handles. Shared manifests and generated
loader files stay with their designated owners. The original acceptance criteria
are:

- `navigation does not orphan resources`
- `The named outcome has actual source/build-bound evidence; unsupported, pending, failed and unperformed checks remain explicit.`

The bounded Rust literal was extracted and compared byte-for-byte with this
Markdown, formatted and checked with the pinned root rustfmt, compiled with the
pinned Rust 2024 compiler using `-Dwarnings`, and executed. Its passing valid and
refusal cases demonstrate the illustrative ownership contract only. Integrated
`df-web` routing, actual browser handle release, reconnect, and independent
frontier review remain pending because no `df-web` runtime exists in this input
revision. Browser/WASM compilation, visual evaluation, and production behavior
were not performed. No Cargo or production qualification claim is made.
