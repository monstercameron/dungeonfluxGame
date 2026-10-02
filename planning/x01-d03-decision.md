# Media trust and decoder bounds

Date: 2026-10-01  
Task/attempt: `B-X01-D03/a1`  
Input revision: `308920e328ba6df85bea1e3d2abcbea5f1b796e6`  
Status: source-backed decision and bounded contract example; production isolation is unimplemented and unqualified

## Decision

Treat every uploaded, imported, cached-for-reprocessing, or provider-returned media byte sequence as hostile. Metadata, filename extensions, declared MIME types, source reputation, and an authenticated uploader do not make bytes safe to decode. `df-media` owns the admission decision and decoder-job lifecycle; `df-assets` owns immutable bytes and manifests; `df-server` owns native process construction and sandbox configuration. `df-media` must dispatch decoding only to a dedicated native worker subprocess with an unprivileged identity, no credentials, no network, and read-only restricted filesystem access. The worker receives only the admitted input and a bounded output destination. It does not share the actor process, provider dispatcher, database credentials, or arbitrary filesystem access. A platform/configuration unable to establish the required isolation refuses decoding and leaves the media unavailable; it must not fall back to in-process decoding.

Admission is finite and precedes allocation or decode. Reuse the service-operation envelope for imported bytes: at most 32 MiB compressed input and at most 64 MiB decompressed input. Apply both limits while streaming; reject an oversized declared length before accepting the body, and stop as soon as observed bytes exceed a limit. Do not decompress archives or follow archive paths for media decode. The existing native decoder proposal in `service-operations.md` supplies candidate per-job limits: two threads, 256 MiB memory, 10 seconds CPU, 15 seconds wall time, 4096 by 4096 image dimensions (16 million pixels), video up to 8 seconds and 240 frames, and 64 MiB output. These are policy ceilings from the source plan, not measured capacities or a claim that every platform can enforce each limit. Use checked arithmetic for dimensions, frame counts, and byte accounting.

Accept only explicitly supported passive formats selected by the owning media contract. Reject executable or active formats including HTML and SVG, unsupported codecs before decode, malformed/truncated inputs, limit violations, and disagreement between approved manifest MIME/type metadata and sniffed/decoded format, dimensions, duration, or frame count. A decoder crash, timeout, memory limit, sandbox setup failure, or malformed output is a typed failed/unavailable result. It never yields a partial ready asset. Retain a secret-free failure class and job/build/configuration identity; do not log raw media, credentials, or private speech. Failure of optional media does not block game state or substitute fabricated success.

The contract example below is a finite, std-only decision model. It exercises accepted admission and meaningful refusals; it does not spawn a process, enforce an OS sandbox, inspect real bytes, or define a production API. Input facts in the model stand in for a trusted bounded streaming ingress and format probe. Production must ensure those facts cannot be supplied by the untrusted client as if verified.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Png,
    Webp,
    Mp4,
    Svg,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
    IsolationUnavailable,
    ProcessLimitExceeded,
    CompressedInputTooLarge,
    DecompressedInputTooLarge,
    UnsupportedFormat,
    ActiveFormat,
    ImageDimensionsExceeded,
    VideoDurationExceeded,
    VideoFrameLimitExceeded,
    OutputLimitExceeded,
    ManifestMismatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Admission {
    Decode,
    Refuse(Refusal),
}

#[derive(Clone, Copy)]
struct Sandbox {
    separate_process: bool,
    unprivileged: bool,
    network_disabled: bool,
    filesystem_read_only: bool,
    threads: u8,
    memory_mib: u16,
    cpu_seconds: u8,
    wall_seconds: u8,
}

#[derive(Clone, Copy)]
struct Probe {
    sandbox: Sandbox,
    compressed_bytes: u64,
    decompressed_bytes: u64,
    format: Format,
    manifest_matches_probe: bool,
    width: u32,
    height: u32,
    duration_millis: u32,
    video_frames: u32,
    output_bytes: u64,
}

fn admit(p: Probe) -> Admission {
    if !p.sandbox.separate_process
        || !p.sandbox.unprivileged
        || !p.sandbox.network_disabled
        || !p.sandbox.filesystem_read_only
    {
        return Admission::Refuse(Refusal::IsolationUnavailable);
    }
    if p.sandbox.threads > 2
        || p.sandbox.memory_mib > 256
        || p.sandbox.cpu_seconds > 10
        || p.sandbox.wall_seconds > 15
    {
        return Admission::Refuse(Refusal::ProcessLimitExceeded);
    }
    if p.compressed_bytes > 32 * 1024 * 1024 {
        return Admission::Refuse(Refusal::CompressedInputTooLarge);
    }
    if p.decompressed_bytes > 64 * 1024 * 1024 {
        return Admission::Refuse(Refusal::DecompressedInputTooLarge);
    }
    if p.format == Format::Svg {
        return Admission::Refuse(Refusal::ActiveFormat);
    }
    if p.format == Format::Unknown {
        return Admission::Refuse(Refusal::UnsupportedFormat);
    }
    if !p.manifest_matches_probe {
        return Admission::Refuse(Refusal::ManifestMismatch);
    }
    if matches!(p.format, Format::Png | Format::Webp)
        && (p.width > 4096
            || p.height > 4096
            || u64::from(p.width) * u64::from(p.height) > 16_000_000)
    {
        return Admission::Refuse(Refusal::ImageDimensionsExceeded);
    }
    if p.format == Format::Mp4 && p.duration_millis > 8_000 {
        return Admission::Refuse(Refusal::VideoDurationExceeded);
    }
    if p.format == Format::Mp4 && p.video_frames > 240 {
        return Admission::Refuse(Refusal::VideoFrameLimitExceeded);
    }
    if p.output_bytes > 64 * 1024 * 1024 {
        return Admission::Refuse(Refusal::OutputLimitExceeded);
    }
    Admission::Decode
}

fn main() {
    let valid_png = Probe {
        sandbox: Sandbox {
            separate_process: true,
            unprivileged: true,
            network_disabled: true,
            filesystem_read_only: true,
            threads: 2,
            memory_mib: 256,
            cpu_seconds: 10,
            wall_seconds: 15,
        },
        compressed_bytes: 1024,
        decompressed_bytes: 4096,
        format: Format::Png,
        manifest_matches_probe: true,
        width: 1280,
        height: 720,
        duration_millis: 0,
        video_frames: 0,
        output_bytes: 2048,
    };
    assert_eq!(admit(valid_png), Admission::Decode);
    assert_eq!(
        admit(Probe {
            format: Format::Webp,
            ..valid_png
        }),
        Admission::Decode
    );

    assert_eq!(
        admit(Probe {
            sandbox: Sandbox {
                unprivileged: false,
                ..valid_png.sandbox
            },
            ..valid_png
        }),
        Admission::Refuse(Refusal::IsolationUnavailable)
    );
    assert_eq!(
        admit(Probe {
            sandbox: Sandbox {
                network_disabled: false,
                ..valid_png.sandbox
            },
            ..valid_png
        }),
        Admission::Refuse(Refusal::IsolationUnavailable)
    );
    assert_eq!(
        admit(Probe {
            sandbox: Sandbox {
                memory_mib: 257,
                ..valid_png.sandbox
            },
            ..valid_png
        }),
        Admission::Refuse(Refusal::ProcessLimitExceeded)
    );
    assert_eq!(
        admit(Probe {
            compressed_bytes: 32 * 1024 * 1024 + 1,
            ..valid_png
        }),
        Admission::Refuse(Refusal::CompressedInputTooLarge)
    );
    assert_eq!(
        admit(Probe {
            decompressed_bytes: 64 * 1024 * 1024 + 1,
            ..valid_png
        }),
        Admission::Refuse(Refusal::DecompressedInputTooLarge)
    );
    assert_eq!(
        admit(Probe {
            format: Format::Svg,
            ..valid_png
        }),
        Admission::Refuse(Refusal::ActiveFormat)
    );
    assert_eq!(
        admit(Probe {
            manifest_matches_probe: false,
            ..valid_png
        }),
        Admission::Refuse(Refusal::ManifestMismatch)
    );
    assert_eq!(
        admit(Probe {
            width: 4096,
            height: 4096,
            ..valid_png
        }),
        Admission::Refuse(Refusal::ImageDimensionsExceeded)
    );
    assert_eq!(
        admit(Probe {
            output_bytes: 64 * 1024 * 1024 + 1,
            ..valid_png
        }),
        Admission::Refuse(Refusal::OutputLimitExceeded)
    );
    assert_eq!(
        admit(Probe {
            format: Format::Mp4,
            duration_millis: 8_001,
            ..valid_png
        }),
        Admission::Refuse(Refusal::VideoDurationExceeded)
    );
    assert_eq!(
        admit(Probe {
            format: Format::Mp4,
            video_frames: 241,
            ..valid_png
        }),
        Admission::Refuse(Refusal::VideoFrameLimitExceeded)
    );
}
```

Admission is not successful decoding or publication. The native worker must enforce its process limits and sandbox at creation, consume bounded streams, and return only fully validated output plus typed status. The caller discards temporary/partial output on every failure. `df-media` owns queue admission, refusal semantics and job status; `df-server` supplies the isolated subprocess/executor; `df-assets` validates and atomically publishes complete immutable bytes/manifests; `df-session` owns any committed run job and stale-result fencing. `df-api`/`df-providers` ingress adapters must use the same finite limits before buffering. None may introduce a competing trust or decoder policy.

## Alternatives

- **Decode hostile input inside the actor/server process:** rejected because parser memory corruption, hangs, or resource exhaustion would share credentials and authority with game/session work.
- **Rely on MIME strings, extensions, a clean scanner result, or a provider's origin:** rejected because those claims do not constrain parser behavior or decoded size. Sniff and validate bytes inside the isolated worker after bounded ingress.
- **Run a decoder container/process without a mandatory isolation check:** rejected because a process boundary alone does not remove credentials, network, filesystem, or resource authority. Unsupported isolation must refuse.
- **Accept broad or unlimited media for compatibility:** rejected; limits must hold before and after expansion and output generation. Better support requires a separately reviewed bound and representative capacity evidence.

## Owners, source basis, and unresolved gates

The existing architecture names `df-media` as the Asset Engine and runtime admission/queue/request/error owner, `df-assets` as immutable bytes/metadata owner, and `df-server` as process composition. The cross-system coordinator owns this decision-to-isolated-process integration and acceptance hook. This decision adds no crate, shared type, provider, or persisted schema. Exact Rust types, OS sandbox mechanism, process supervisor, filesystem namespace, UID allocation, syscall policy, and build/deployment configuration remain later implementation decisions with their crate owners.

The bounds above reuse [service operations](service-operations.md): 32 MiB import, 64 MiB decompressed input, and the decoder proposal of two threads, 256 MiB memory, 10 seconds CPU, 15 seconds wall time, image max 4096x4096/16 million pixels, video max 8 seconds/240 frames, and 64 MiB output. The proposal is explicitly unmeasured. Qualify enforcement and headroom on each supported native platform and representative adversarial corpus before production admission. Memory/CPU values must not be treated as achieved isolation until verified by the actual launcher and OS. Browser/WASM decoding is not authorized by this native decoder policy; device support remains untested.

The governing plans also require authorized native allowlist fetching for approved remote retrieval; this decision does not add URL fetching or let provider-supplied URLs bypass that boundary. Client uploads are bounded, while authorization/tenant/audience checks remain the relevant auth/API owner’s responsibility. A media job must not become a private asset disclosure path.

Unresolved production gates: concrete decoder and codec set; MIME sniffer and parser versions; supported OS sandbox primitives and kernel/container policy; actual cgroup/job-object/resource enforcement and process kill/reap behavior; pool/concurrency and fair queue limits from load testing; accounting accuracy for compressed/decompressed/duration/frame/output bounds; adversarial format corpus, fuzzing and crash recovery; temp-file lifecycle and complete publication; tenant/audience authorization; telemetry schema/retention for classified decoder outcomes; availability/fallback behavior on each deployment target; and independent integrated review. This task does not qualify any sandbox, decoder, phone/browser, provider, or deployed workload. No process-spawning prototype or paid call was made.

## Source and verification status

Governing input hashes and exact executable-check receipts are retained under `development/evidence/fanout-20261001/wave-02/B-X01-D03/`. The standalone example is illustrative policy logic only; successful compilation/execution does not establish a process sandbox or runtime media behavior. The original acceptance is `isolated unprivileged process`; this design decision specifies that required boundary and explicit refusal semantics, but actual build/deployment-bound process evidence remains unperformed. Independent frontier review and coordinator integration remain mandatory.

Original acceptance criteria (preserved verbatim):

1. `isolated unprivileged process`
2. `The named outcome has actual source/build-bound evidence; unsupported, pending, failed and unperformed checks remain explicit.`

Original verification procedures (preserved verbatim):

1. `Freeze the cited source decision and a bounded contract example; compare isolated unprivileged process. Retain decision, alternatives and unresolved facts.`
2. `Exact executable commands: TBD at G01 and scoped prerequisite resolution; this planned procedure is not a claim that Rust/browser/provider checks ran.`
