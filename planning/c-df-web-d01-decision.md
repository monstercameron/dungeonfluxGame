# Rust/WASM boot and role mounting decision

Date: 2026-10-02
Task: `B-C-df-web-D01-a1`
Input: `4c0e050c2d30e632bd3b32c51a92cfda52479e6e`

## Decision

`df-web` owns one Rust/WASM shell for the page lifetime. Startup validates the
build identity and required browser/runtime wiring before exposing a usable
shell; a required failure produces a visible startup failure and leaves the
application unavailable rather than presenting a partial success. Generated
JavaScript may load the WASM module and provide browser bindings. It contains no
application or UI decisions. The Rust composition root selects and mounts a
player, shared-display, or approved panel target in the existing shell.

The shell owns the active route scope and its listeners, subscriptions, pending
asset/decode work, animation handles, and render resources. Replacing a scope
first disposes that scope exactly once; route, phase, view, and reconnect updates
preserve the shell and reconcile mounted Rust components in place. Disposal is
idempotent at the shell boundary. Late completions must match the active
generation and resource key before publishing. A fatal required startup error
is surfaced safely; an optional renderer or asset may use an explicitly
permitted fallback while input and the error surface remain usable.

`Role` and route selection are presentation inputs only. They carry no identity,
membership, capability, or audience grant. `df-auth`/server-projected authorized
views remain the authority; host controls and private panels are rendered only
when a separately validated permission is present. A URL or host-looking route
cannot mint that permission. The shell does not import server/game authority or
provider crates.

Framework, browser binding, concrete boot DTOs, generated loader layout, and
concrete route/panel registry remain open at their existing G01/G04/G08 and
G02/source-boundary gates. This decision names ownership and failure semantics;
it does not freeze a framework-specific API or claim a production boot exists.

## Bounded Rust contract example

This standalone example models boot validation, in-place scope replacement, and
owned disposal. It is a contract illustration, not a `df-web` implementation or
browser qualification. Production scopes must own the actual browser listeners,
subscriptions, tasks, and renderer/audio handles and apply generation/key checks
to asynchronous completions.

```rust
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Role {
    Player,
    SharedDisplay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Panel {
    CustomerService,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MountTarget {
    Role(Role),
    Panel(Panel),
}

#[derive(Debug, Eq, PartialEq)]
enum StartupError {
    MissingBuildIdentity,
    EmptyBuildIdentity,
    ShellDisposed,
}

struct ResourceOwner(Arc<AtomicUsize>);

impl Drop for ResourceOwner {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

struct MountedScope {
    target: MountTarget,
    _resources: ResourceOwner,
}

struct Shell {
    build_identity: String,
    mounted: Option<MountedScope>,
    disposed: bool,
}

impl Shell {
    fn boot(build_identity: Option<&str>) -> Result<Self, StartupError> {
        let build_identity = build_identity.ok_or(StartupError::MissingBuildIdentity)?;
        if build_identity.is_empty() {
            return Err(StartupError::EmptyBuildIdentity);
        }
        Ok(Self {
            build_identity: build_identity.to_owned(),
            mounted: None,
            disposed: false,
        })
    }

    fn mount(
        &mut self,
        target: MountTarget,
        resources: ResourceOwner,
    ) -> Result<(), StartupError> {
        if self.disposed {
            return Err(StartupError::ShellDisposed);
        }
        self.mounted = Some(MountedScope {
            target,
            _resources: resources,
        });
        Ok(())
    }

    fn target(&self) -> Option<MountTarget> {
        self.mounted.as_ref().map(|scope| scope.target)
    }

    fn dispose(&mut self) {
        self.mounted = None;
        self.disposed = true;
    }
}

fn main() {
    let released = Arc::new(AtomicUsize::new(0));

    assert_eq!(Shell::boot(None).err(), Some(StartupError::MissingBuildIdentity));
    assert_eq!(
        Shell::boot(Some("")).err(),
        Some(StartupError::EmptyBuildIdentity)
    );

    let mut shell = Shell::boot(Some("build-17")).expect("valid fixture identity");
    assert_eq!(shell.build_identity, "build-17");
    shell
        .mount(
            MountTarget::Role(Role::Player),
            ResourceOwner(Arc::clone(&released)),
        )
        .expect("first mount");
    assert_eq!(shell.target(), Some(MountTarget::Role(Role::Player)));
    shell
        .mount(
            MountTarget::Role(Role::SharedDisplay),
            ResourceOwner(Arc::clone(&released)),
        )
        .expect("replace mounted role in the same shell");
    assert_eq!(released.load(Ordering::SeqCst), 1);
    assert_eq!(shell.target(), Some(MountTarget::Role(Role::SharedDisplay)));

    shell.dispose();
    shell.dispose();
    assert_eq!(released.load(Ordering::SeqCst), 2);
    assert_eq!(shell.target(), None);
    assert_eq!(
        shell.mount(
            MountTarget::Panel(Panel::CustomerService),
            ResourceOwner(Arc::clone(&released)),
        ),
        Err(StartupError::ShellDisposed),
    );
    assert_eq!(released.load(Ordering::SeqCst), 3);
    println!("PASS: valid boot, role replacement, and exactly-once disposal");
    println!("PASS: missing/empty identity and disposed-shell refusals");
}
```

The example exercises valid boot/mount/replacement/disposal and startup/terminal
refusals. The `Role` enum contains no permission grant; authorization must be
provided through a separate validated server view or client authorization
boundary before protected controls are mounted. The counter is only a bounded
fixture for drop behavior, not a production resource metric.

## Alternatives and unresolved facts

- A full-page reload per route/update would destroy focus, drafts, subscriptions,
  and valid decoded resources; it conflicts with the persistent shell contract.
- Handwritten JS/TypeScript composition would put application behavior outside
  the Rust client architecture. Generated loader and browser-binding glue is the
  sole allowed JavaScript boundary.
- Treating `/host` or `Role::SharedDisplay` as authorization would conflate
  presentation with access and expose privileged controls without an independent
  grant; reject this design.
- The existing `df-tools` browser fixture is a transport qualification harness.
  Its current `#[wasm_bindgen(start)]`, fixture button callback, `Closure::forget`,
  and `Document` result rendering are page-lifetime fixture mechanics, not a game
  shell, route registry, or production disposal contract.
- No `df-web`, `df-player`, or `df-display` runtime crate source exists in this
  revision. Browser/WASM compilation, DOM mount/dispose, reconnect, actual routes,
  CustomerService integration, and visual evaluation therefore remain pending.
  G02 bridge feasibility and existing `df-client`/`df-ui` boundaries are also
  prerequisites; no framework or production API is selected here.

## Evidence and handoff

The decision is grounded in `planning/subsystem-architecture.md` (Browser
crates), `planning/subsystem-interfaces.md` (Common contract rules, Identity and
session ownership, Browser interfaces), `planning/client-architecture.md`,
`planning/client-presentation.md`, `planning/commerce-service.md` (Mounted Rust
customer experience), and the scoped existing `crates/df-tools/src/browser.rs`
and `crates/df-tools/src/lib.rs` sources, all at the frozen input revision above.
The detailed file hashes and check receipts are in this attempt's immutable
evidence manifest.

The next actual consumer is `df-web` startup/composition, mounted beside `df-player`
and `df-display`, with `df-client`/`df-ui` supplying the already planned adapters
and panels. The minimum source paths for that implementation are the owning
`crates/df-web` crate/build configuration plus the approved consumer interfaces;
shared workspace/build manifests require their designated owner. A real browser
build and two-role smoke test must then demonstrate in-place updates, safe startup
failure, separately authorized host/panel access, reconnect, and single disposal.

This document and its bounded executable example do not show a running game UI,
production `df-web` behavior, or device compatibility. Independent frontier review
and coordinator integration remain outstanding.
