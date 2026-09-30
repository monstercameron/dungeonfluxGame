# Commercial validation and income model

Status: candidate experiment and reproducible offline sensitivity model. No customers, conversion, retention, CAC, throughput or profit have been observed.

## Offer, activation and launch evidence

Preserve the entire 47-feature, full-standard-2024, Rust thin-client remote/living-room product. A focused early validation campaign is a delivery/test slice, not permission to remove the final required scope. Initial proposition: existing groups can play source-correct, persistent cinematic campaigns with natural NPCs and private phones without installing rules logic or requiring a dedicated expert human DM. Novelty is a hypothesis; current official competitor evidence already shows overlapping voice, AI GM and shared-screen functionality.

Test the existing $49/$59 campaign offer against a host bundle at matched aggregate play/AI allowances and optional quoted prepaid video. Do not sell unlimited sessions, video or group size before qualification. A campaign purchase covers the group under qualified concurrent-device usage, rather than per-seat tax; heavy parallel/private narration still consumes disclosed allowance. PriceVersion/EntitlementSnapshot disclose included units, hard stop/fallback, extra-use quote, rights/content availability, cancellation/refund and retention. A missing right to use the full required catalog blocks full-game commercial launch, not a reason to claim the SRD subset is full 2024 support.

Owned staged experiments (coordinator product owner, future actual-output frontier evaluator): (1) observe 12 existing group hosts complete onboarding and a fixed mixed-room session; (2) offer matched price/allowance choices to 30 qualified hosts, record explicit purchase intent and reasons, no hypothetical willingness-to-pay presented as revenue; (3) only after working qualified game and commercial authorization, follow at least 30 paid campaigns for four weeks, with anonymized activation, second-session, weekly repeat, canceled/retained and support load; (4) a bounded acquisition cohort with approved spend produces CAC per *new retained paying campaign*, not click cost. These sample counts yield exploratory evidence with uncertainty, not statistical market proof. No invitations/contact/spend are authorized or performed in the current planning task.

Definitions: activation = host plus at least two distinct players finish a source-valid supported encounter and checkpoint; time-to-first-play from first account creation excludes neither setup errors nor failed joins; retained paying = paid-through active and a second completed session within28days; voluntary cancellation and payment-failure churn reported separately; refund rate by charged gross dollars and cohorts, not only accepted sessions. Product telemetry records consented aggregate milestones with tenant scoped pseudonymous IDs, no private speech/content. Track p50/p95 onboarding, activation/return/dropoff reasons, transport/audio failures, prepared fallback use and support minutes per campaign. Candidate progression thresholds: >=80% setup success, median<=10min to first action, >=60% four-week paid repeat, >=70% contribution margin before acquisition/fixed overhead, <=6months CAC payback and enough cohort size to show intervals. If below target, fix measured bottleneck or change offer; do not market as proven profitable. Root's provisional **$50,000/month** is modeled gross recognized service revenue, not income/profit or an achieved target.

## Rights-complete launch gate

Each required catalog/rule/monster/art/text and generated-media source has `RightsGrant` identity, licensor/evidence, permitted commercial use/distribution/derivatives/territory/term/attribution, source/catalog digest and revocation expiry. G07 plus commercial owner inventories every required family and determines a reviewed lawful distribution strategy before sale. SRD5.2.1 CC-BY applies to its specific contents, not all free D&D Beyond pages or every 2024 book. The official Creator FAQ explicitly distinguishes SRD from Basic Rules and discusses exclusions. A book purchase, personal possession, model memory or creator import is not treated as a blanket commercial redistribution grant. Full catalog requirement stays open until evidence supports it; legal review/license cost enters fixed and per-campaign model, not assumed zero. [Official SRD](https://www.dndbeyond.com/srd), [official Creator FAQ](https://www.dndbeyond.com/creator-faq).

`RightsPolicy::admit(source_digest, intended_use, tenant, territory, at_time)` validates supplied reviewed grants before packaging/generation/delivery/export; content importer can't mint grants. Expired/revoked grants stop new affected admissions, retain minimal audit, restrict lawful archived access according to actual terms and propose source-compatible migration/credit remedy without silently changing mechanics. Prepared cache does not bypass rights. Marketing D&D trademarks, generated voice/style/licensed likeness, minors/privacy, payment/tax/cancellation market terms require separately reviewed rights/policy evidence; current doc is an engineering boundary, not legal clearance.

## Reproducible economics and cash

`development/service-economics.py` computes Decimal scenarios from explicit JSON inputs and emits results plus model identity. It treats subscription revenue excluding sales tax, refunds and uncollectible invoices; gross paid video top-ups are cash and a deferred credit liability until consumed. Earned credit revenue subtracts supplier cost and payment fees already allocated to top-up; no double count. Failed-but-billed provider attempts multiply raw usage; unknown dispatch reserves affect available cash/risk and are reported separately, never invented net profit. Full source licensing, relay/retention/telemetry, paid support labor, salaries/engineering agents/admin, fixed production infrastructure, customer acquisition and churn replacement are explicit.

Formulas: net service revenue = recognized gross - refunds - bad debt; sales tax collected is payable liability, not revenue; monthly contribution = net service revenue - processing/Billing/dispute fees - generation/storage/relay/support/variable-rights costs; operating result = contribution - fixed operating cost - replacement/new cohort acquisition. CAC replacement = active campaigns * monthly churn * CAC. Payback = CAC / monthly per-campaign contribution (only if positive). Subscriber count for $50k recognized subscription revenue uses ceil(target/price) before refunds. Growth acquisition uses new retained cohorts separately; monthly churn compounds, no static customer base assumed perpetual. Cash receipts include tax/top-ups; subtract tax remittance, refunds, paid costs, acquisition and additions to withheld unknown liability; deferred credit outstanding shown as obligation. Corporate income tax is an explicit scenario rate on positive operating result; statutory applicability/legal accounting remains reviewed, not predicted.

Baseline rate snapshot2026-09-29 retains existing approximate raw nonvideo $1.5565/session and billed-attempt multiplier1.2. Candidate new service scenario uses 4sessions, $49/$59, support **20min/campaign/month at $30/hour**, variable delivery/rights **$2/campaign/month**, fixed operations **$8,000/month**, CAC **$80**, churn **5%/month**, refunds2%, bad debt1%, sales tax0% baseline (8% stress), effective income tax20% illustrative, payment3.6%+$0.30, disputes0.5% of charges and $30 combined candidate dispute/counter fee. These are assumptions, not vendor quotes or actual headcount costs. Stripe currently lists US domestic card2.9%+$0.30 and Billing pay-as-you-go0.7%; dispute/counter fees can differ by handling and market, so qualification pins actual pricing. [Stripe pricing](https://stripe.com/pricing).

Run low/high support, doubled usage/failure waste,10% churn/CAC160, higher rights/fixed cost, tax-inclusive versus tax-exclusive offer, prepaid unused-credit and unknown-liability cases. Output target/break-even campaign counts and runway cash sensitivity, not a single rosy gross margin. The prior $2 support reserve/$100 shared VM illustration is superseded for company-income planning by this explicit service labor/operations scenario; it remains a historical per-session comparison. No numeric result establishes demand, adequate hardware, lawful catalog or paid conversion.

Economic accounting assumptions are explicitly illustrative: disputed principal is
an additional nonoverlapping loss beyond refunds/bad debt; fixed payment fees use
an explicit successful/charged transaction count (baseline conservatively one
per active campaign), not an inference from dollar bad debt. Refunded subscription
sales tax is assumed returned in full under the example jurisdiction; chargeback
sales tax is likewise fully reversed and remittance reduced in the toy model. Credit tax handling is zero
in this illustration and requires jurisdiction-specific qualification. Reports
expose collected/refunded/remitted tax and complete cash identity. Consolidated
operating profit and illustrative income tax include earned credit margin and
all top-up fees; unused credit remains deferred liability with100% cash reserve.
Unknown supplier liability is a held reserve, never an incurred expense or
physical loss of bank cash. Liquid cash and available unreserved cash differ.
The break-even count is explicitly steady-state subscription-only, excluding
new-growth CAC and credit earnings; it is not production-capacity evidence.

A reviewed lawful representation route may include licensed SRD expression,
independently authored expression of mechanics, and separately permitted book text,
art/lore/trademarks/provider processing. Ideas/systems and original expression have
different legal treatment; the rights ledger records the selected reviewed basis,
not an assumption every mechanic requires a paid license. Any rights fee is unknown
until that route is reviewed. No copying or full2024 commercial clearance follows
from this nuance. [US Copyright Office FAQ](https://www.copyright.gov/help/faq/faq-protect.html).

Bank-only stationary burn projection is explicitly not reserve-aware runway and
cannot extrapolate finite opening credits indefinitely. Report opening/ending
available cash and its change/shortfall separately; negative available cash blocks
new paid admissions even if bank receipts rose. Future growth/cash forecasts need
a month-by-month cohort/liability schedule before any actual runway promise.
