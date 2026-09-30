# Campaign pricing and cost controls

Research snapshot: **2026-09-29**. USD, public list prices, before taxes and
negotiated discounts. This is a pricing experiment and editable cost model, not a
capacity benchmark or a promise of profitability. No provider calls were purchased.

## Recommendation

Test **$49–$59 per active campaign per month**, with a bounded nonvideo allowance
roughly matching four three-hour sessions. Players join the campaign without an
invented four-player limit; actual supported membership/concurrency still needs
measurement. Additional play and private/parallel AI usage consume an explicitly
priced allowance. Charge **optional video separately through prepaid credits**,
with a displayed quote and host-approved campaign spend cap. Avoid unlimited live
video in any fixed-price subscription.

Use economical short scene clips, prepared loops, still images with smooth Rust
client animation, and reusable campaign assets as the normal visual presentation.
Offer premium video as an occasional cinematic. A three-hour session does not
need three hours of freshly generated video. These are product recommendations,
not changes to the authoritative server or thin-client contracts.

The later competitive review documents host subscriptions around $20–$40 that
include several campaigns. The $49–$59 per-campaign offer needs willingness-to-pay
and retention evidence against equal-usage alternatives; the margin calculation
below does not establish a viable market price. Compare campaign, host-bundle and
session offers before fixing the customer allowance. See [competitive research](competitive-research.md).

## Verified rate inputs

The pages below were opened on the snapshot date; their prices generally have no
explicit effective date. Recheck before offering a customer price, and pin the
actual provider/model, account access, limits and output specification in each
generation quote.

| Provider/product | Published rate used | Scope |
| --- | --- | --- |
| OpenAI GPT-6 Luna | $0.10 input / $0.50 output per million tokens | Standard, short context; cache read $0.01, write $0.125 |
| OpenAI GPT-6.1 Sol | $2 input / $10 output per million tokens | Standard, short context; cache read $0.10, write $2.50 |
| OpenAI mini transcription | approximately $0.003/minute | `gpt-4o-mini-transcribe`; estimate, reconcile actual token billing |

Long-context and faster processing tiers cost differently. The baseline deliberately
assumes no prompt-cache discount. Narrative quality and streaming latency still need
evaluation. These are API prices, independent of developer-agent subscriptions.
[Official OpenAI API pricing](https://developers.openai.com/api/docs/pricing).

| Google product | Published rate used | Scope |
| --- | --- | --- |
| Veo 3.1 Lite | $0.05/second at 720p; $0.08 at 1080p | Preview, video with audio |
| Veo 3.1 Fast | $0.10/second at 720p; $0.12 at 1080p; $0.30 at 4K | Preview, video with audio |
| Veo 3.1 Standard | $0.40/second at 720p/1080p; $0.60 at 4K | Preview, video with audio |
| Gemini Omni 1.1 Flash | about $0.10/second at 720p output | Generally available; input tokens charged separately |
| Gemini 3.1 Flash TTS | $1/million text input; $20/million audio output tokens | Preview; 25 audio tokens/second gives $0.03/output minute |
| Gemini 3.1 Flash Lite Image | $0.0336/1K output image, plus input/thinking | Standard; comparison, not baseline selection |

Google states Veo charges only successfully generated videos. Successfully generated
but rejected clips still need a regeneration allowance. Account access, durations,
reference-frame requirements and latency are unverified; do not assume all models
are interchangeable. [Official Gemini API pricing](https://ai.google.dev/gemini-api/docs/pricing).

Runway API credits are $0.01 each. `gen4_turbo` is 5 credits/output second ($0.05);
`gen4.5` is 12 ($0.12). `gen4_image_turbo` is 2 credits/image ($0.02);
`gen4_image` is 5 at 720p or 8 at 1080p ($0.05/$0.08). Reference inputs,
professional formats, audio and model-specific minimums may change the quote;
this report does not assume video-model feature parity or a failed-job refund.
[Official Runway API pricing](https://docs.dev.runwayml.com/guides/pricing/).

**Exclude Sora from a new deployment.** OpenAI's deprecation table lists the Videos
API and Sora 2 models as removed on **2026-09-24**, before this research date.
Lingering model/changelog pages are not evidence of continued availability.
[Official OpenAI deprecations](https://developers.openai.com/api/docs/deprecations).

## Server, database and delivery

DigitalOcean examples are a $24/month 4 GiB/2 vCPU Droplet with 4,000 GiB transfer,
or $48/month 8 GiB/4 vCPU with 5,000 GiB. These are shared-CPU examples, not proven
game capacities. Backups, volumes, transfer overages and HA are extra.
[Official Droplet pricing](https://www.digitalocean.com/pricing/droplets).

Managed PostgreSQL examples list $15.15/month for 1 GiB/1 vCPU and $30.45 for
2 GiB/1 vCPU, with storage listed at $0.215/GiB-month in 10 GiB increments.
Confirm the selected-region checkout, storage and standby charges; one small node
is not a production HA architecture.
[Official managed database pricing](https://www.digitalocean.com/pricing/managed-databases).

R2 Standard storage is $0.015/GB-month, writes $4.50/million and reads
$0.36/million, with zero R2 Internet egress charges. Monthly included allowances
and billable-unit rounding apply. Origin upload/relay transfer, authenticated
delivery compute, transformation and transcoding can still cost money.
[Official R2 pricing](https://developers.cloudflare.com/r2/pricing/).

Start the business model with a **$100/month infrastructure envelope**, covering
one small Rust server, managed Postgres, backup/retention and headroom. This is an
assumption, not a vendor bundle or production quote. Replace it with actual invoices
and capacity tests; a reliable multi-instance deployment may cost materially more.
Keep telemetry SQLite and its durable spool on a protected volume, with measured
write/retention/backup overhead. For illustration, 100 events/second at 1 KiB each
produce about 1.11 GB in three hours before indexes/WAL/backups. Unsampled enabled
logs require deliberate retention, bounded ingestion and visible pressure.

The current Asset RPC contract streams authorized bytes through the Rust server.
R2 can be its durable backing store; R2's free egress does **not** remove server
relay egress or per-client processing. Direct authorized object delivery would need
an explicit contract/design decision and privacy checks before being counted as a
saving. Do not silently change transport to achieve the forecast.

## Editable economics

`outputs/dungeonflux-pricing-model.json` contains rates, assumptions, formulas and
computed scenarios. Model currencies and units explicitly; monetary implementation
must use exact integer/fixed-point amounts, not these illustrative floating values.

For one session:

```text
raw_generation = text + aggregate_transcription + generated_speech + images + video
generation_cogs = raw_generation × billed_usage_multiplier
monthly_cogs = sessions × generation_cogs + monthly_infra / paying_campaigns
               + support_reserve + storage_and_other_reserve
margin = (price - monthly_cogs - percentage_payment_fees × price - fixed_fee) / price
minimum_price = (monthly_cogs + fixed_fee) / (1 - percentage_payment_fees - target_margin)
```

US domestic Stripe card processing is 2.9% + $0.30 per transaction. Stripe Billing
pay-as-you-go adds 0.7% of Billing volume. The subscription example uses **3.6% +
$0.30**. International cards, FX, refunds/disputes, tax tooling and different
markets require their actual fees. Batch credit purchases to reduce fixed fees.
[Official Stripe pricing](https://stripe.com/pricing).

Base session assumptions: three hours; 400,000 text input and 60,000 total billable
output tokens; 95% Luna / 5% Sol by token workload; 90 **aggregate submitted** audio
minutes; 30 newly generated narration minutes with 10,000 TTS input tokens; 12 new
economical images. The text and voice workloads are hypotheses. System prompts,
repeated context, hidden reasoning tokens, moderation and tool calls count when
billed. A 20% multiplier represents billed re-generations and contingency, not a
claim that failed calls are always charged.

| Nonvideo item | Raw cost per example session |
| --- | ---: |
| Blended text | $0.1365 |
| Transcription | $0.27 |
| Narration + TTS text input | $0.91 |
| Images | $0.24 |
| Total | $1.5565 |
| With 20% contingency | $1.8678 |

Four sessions/month, $100 infra shared across 100 paying campaigns, $2/month support
reserve and $1/month storage/other reserve yield **$11.4712 monthly nonvideo COGS**.
The support reserve is a placeholder to replace with measured labor cost.

| Campaign price/month | Contribution margin after stated fees/reserves |
| --- | ---: |
| $39 | 66.2% |
| $49 | 72.4% |
| $59 | 76.4% |

At 70% target contribution margin, this scenario needs at least **$44.59/month**.
At only ten paying campaigns with the same $100 infra, the minimum rises to
**$78.68**. At 100 paying campaigns and $300 infra, it is **$52.16**. Idle campaigns
still allocate fixed costs. These margins exclude salaries beyond the small
support reserve, acquisition, development-agent spending, content/rule licensing,
legal/admin and profit tax. Company profitability needs those operating costs too;
this is not a revenue/profit forecast.

## Optional video sensitivity

Assume eight-second clips and three-hour play. These are illustrative counts;
each adapter quotes its supported duration and minimums, not arbitrary seconds.
All numbers below are **new generated seconds once per campaign**, before the 20%
billed-usage contingency. Replaying a completed asset incurs delivery, not generation.

| New clips/session | New seconds | Lite 720p $0.05/s | Fast 720p $0.10/s | Standard $0.40/s |
| --- | ---: | ---: | ---: | ---: |
| 6 (2/hour) | 48 | $2.40 | $4.80 | $19.20 |
| 18 (6/hour) | 144 | $7.20 | $14.40 | $57.60 |
| 45 (15/hour) | 360 | $18.00 | $36.00 | $144.00 |

A hypothetical 3× markup on **buffered** generation cost supports about 63.1%
contribution before per-topup fixed fees/delivery. For six clips, that is $8.64
Lite / $17.28 Fast / $69.12 Standard; for 18, $25.92 / $51.84 / $207.36. Thus an
economical session with six Lite clips plausibly sells near **$8–$12 in video
credits**; premium production is substantially dearer. A 4× buffered-cost markup
targets about 71.4% before fixed fees/delivery. Neither markup is a customer price
commitment; quote actual per-generation specification and benchmark demand.

At four sessions/month, 18 fresh Standard clips/session add **$276.48** buffered
wholesale video cost alone. Folding that into a $49 subscription would lose money.
Generating all 10,800 seconds of a three-hour session would cost $540 Lite or
$4,320 Standard before contingency: continuously fresh video is a different product.

## Population, duration and reuse

Let `N` be supported active players and `D` all receiving player/display devices.
Campaign-wide scene/video and shared narration are generated once. Costs increase
with individual speech, private narration/scenes, action volume, projection work,
DB work and `D` deliveries. Do not multiply shared video generation by `N`, or
claim arbitrarily many seats are free to serve.

- Shared 90-minute submitted speech costs $0.27 regardless of who spoke. Keeping
  every microphone in an always-on billed transcription stream instead costs
  `N × 180 × $0.003`: $4.32 at 8 players, $10.80 at 20, $27 at 50. These population
  examples are cost sensitivities, not supported game capacities. Gate transmission
  and transcription deliberately; push-to-talk must remain an explicit product mode.
- At an assumed 8 Mb/s encoded bitrate, 144 seconds equals 0.144 GB per device;
  50 recipients receive 7.2 GB. Three hours of continuous playback is 10.8 GB/device,
  or 540 GB for 50. Codec/bitrate/request counts must be measured, especially mobile.
- Doubling session duration doubles activity-based cost only if calls, speech and
  scenes scale proportionally. Initial portraits and retained campaign assets can
  be amortized; per-player onboarding and private speech may scale with membership.
- Prepared/public loops may amortize across campaigns only when content rights,
  provider terms and privacy permit. Never share personalized/private prompts or
  outputs through a global cache. Cache by relevant model/spec/content/context/
  locale/voice revisions as required by the current provider contracts.

## Required billing and acceptance work

Reuse `df-provider-api::BudgetStore` and PostgreSQL durable usage records; no new
billing crate is selected by this research. These are explicit follow-up plans:

1. Versioned price catalog with currency, billing unit, duration/resolution/audio/
   references/minimums, effective interval, quote TTL and source. Separate customer
   credit price from supplier estimate and audited actual invoice reconciliation.
2. Host-approved allowance/spend policy and durable campaign wallet. Reserve every
   started paid branch atomically before dispatch, including concurrency/retries;
   release unused funds, reconcile actual/unknown spend and enforce new-job admission.
   Provider overbilling can exceed estimates, so show the discrepancy and stop new
   jobs rather than reporting a fictional hard cap. Authorization gates spending.
3. Idempotent credit purchase, consumption, refund and payment webhook handling.
   Keep payment/provider request IDs and append-only monetary ledger; failed/rejected
   customer-visible results have a documented refund/regeneration policy. Decide who
   bears supplier costs for failed generations instead of silently consuming credits.
4. Unknown paid outcomes retain reservations; no automatic duplicate non-idempotent
   generation after timeout. Cached/prepared/replayed success never fabricates new
   generation charges. Quote approved alternative/fallback before extra spend.
5. OTEL per-job usage/cost/cache/retry/latency and campaign cost projections, plus
   invoice reconciliation. Measure concurrent campaigns, members, source/relay
   bandwidth, DB contention, telemetry lag/disk, retention, codec/transcode cost and
   support workload. Avoid prompt/customer secrets in billing logs and metrics.
6. Before setting launch limits/prices, test economical vs premium narration and
   video quality/latency, P50/P95 session cost, abuse/retries and realistic concurrency.
   Validate provider commercial terms, content licensing, geographic payment fees,
   allowances, refund policy and customer willingness to pay. Test reserve races,
   duplicate webhooks, price expiry, crash/unknown reconciliation and permitted
   private asset reuse with deterministic fakes before any scoped paid test.

This document governs commercial planning alongside [Subsystem interfaces](subsystem-interfaces.md),
[Storage architecture](storage-architecture.md), [Observability](observability.md),
[Client architecture](client-architecture.md) and [Optimization](optimization.md).
The project remains in planning; none of these billing/server behaviors is implemented.

## Service spend and income refinement

[Commerce service](commerce-service.md) supplies the single exact hierarchical
platform/supplier/payer/campaign/job spend and concurrency authority, including
current entitlements, reservations and unresolved liabilities. Every hedged branch
requires its own admitted reservation under the same global cap; cache/prepared
modes reserve no new supplier spend. [Commercial validation](commercial-validation.md)
and the reproducible service-economics model separate net earned service revenue,
credit cash/liability, support, rights, acquisition/churn and fixed operating costs.
The earlier small-server/$2 support contribution illustration is not company profit
or proven capacity. [Service operations](service-operations.md) supplies finite
proposed workload and recovery targets. Paid launch requires the observed evidence,
not just source-backed planning parity.
