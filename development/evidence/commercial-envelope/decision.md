# DungeonFlux commercial decision envelope

The current $49/$59 offers fail the proposed 70% contribution gate at the unchanged
four-session, 20-minute-support workload. The missing decision is a **joint measured
support and generation budget**, rather than a cheaper vendor assumption or more
campaigns. More campaigns do not improve this contribution percentage.

This is offline arithmetic, not customer willingness, actual latency/capacity, rights
clearance or profitable income evidence. Preserve all 47 features, full 2024 rules,
flexible groups and separately quoted video. A subscription allowance is aggregate
usage, never a four-player cap or permission to withhold source-required outcomes.

## Unchanged assumptions and exact constraint

Existing inputs: refunds2%, bad debt1%, separate chargeback principal0.5%; payment
3.6% on99% charged pretax value plus $0.30 per campaign; dispute handling$0.15;
delivery/rights$2; four sessions at raw$1.5565 × billed-attempt multiplier1.2;
support20min at$30/h; fixed operations$8,000; monthly churn5%; CAC$80.
These are hypotheses, including rights cost. New customers and earned video-credit
margin are zero in this stationary subscription comparison.

The existing model defines contribution margin over **net recognized subscription
revenue**, after refunds/bad debt/chargeback principal, before acquisition and fixed
operations. That denominator is already explicit; no gross/net defect was found.
For pretax monthly price P, support minutes M and matched session-equivalents S:

```
Net revenue per campaign = 0.965 × P
Contribution = 0.92936 × P − 2.45 − 1.8678 × S − 0.50 × M
70% net-revenue margin requires:
1.8678 × S + 0.50 × M <= 0.25386 × P − 2.45
```

Session-equivalents use the present workload assumption, not a promised supported
session count. Real session length, group behavior, private conversations, reuse,
failed billed attempts and prepared media can change measured aggregate cost.

| Boundary | $49 | $59 |
| --- | ---: | ---: |
| Joint generation + support budget/campaign/month | $9.98914 | $12.52774 |
| Support ceiling at four modeled sessions | 5.03588 min | 10.11308 min |
| Safe two-decimal support ceiling, rounded down | 5.03 min | 10.11 min |
| Session-equivalent ceiling at 20 support minutes | **Infeasible even with zero generation** | 1.353324767… |
| Present contribution margin | 54.1767% | 61.3174% |
| Maximum CAC for six-month contribution payback | $153.70464 | $209.46624 |

At $49/20min support, the algebraic session ceiling is negative−0.005814327…;
clamping it to zero must not falsely mark the offer qualified. Keeping all present
costs and four sessions/20min support needs exact pretax price
$78.4731741904987…; monetary rounding up gives **$78.48**. This is a sensitivity
boundary, not a recommended price or evidence buyers will accept $79. Taxes,
rights fees, measured service costs and new allowances require recalculation.

## Crosscheckable four-session grid

| Price | Support minutes/month | Contribution/net revenue | Meets proposed70% |
| --- | ---: | ---: | --- |
| $49 | 0 | 75.33% | Yes |
| $49 | 5 | 70.04% | Yes |
| $49 | 10 | 64.75% | No |
| $49 | 20 | 54.18% | No |
| $49 | 40 | 33.03% | No |
| $59 | 0 | 78.88% | Yes |
| $59 | 5 | 74.49% | Yes |
| $59 | 10 | 70.10% | Yes |
| $59 | 20 | 61.32% | No |
| $59 | 40 | 43.75% | No |

The [complete 30-row Decimal grid](dungeonflux-commercial-envelope.json) also spans
2/4/8 session-equivalents, with all actual-model results and unchanged input hashes.
The generating script checks each cell against the independent closed formula;
the frontier evaluator separately verifies arithmetic and boundary sides.

## Income differs from gross revenue and margin

At approximately $50,000 recognized gross subscriptions, the existing populations
1,021@$49 and848@$59 produce illustrative after-tax operating results
$11,257.124992 and$14,570.049536. Income tax20% is a toy effective rate, not legal
advice or an owner's actual personal tax. This operating result is not automatically
cash available for an owner to withdraw; reserves, liabilities and reinvestment apply.

With unchanged positive per-campaign contribution C, churn q, CAC A, fixed cost F,
income-tax scenario t and desired after-tax operating result I:

```
N = ceil((F + I/(1−t)) / (C − q×A))
```

Valid only for a positive denominator and a stationary subscription-only month,
without growth acquisition or credit earnings. Each count below was checked through
the actual Decimal model and the immediately preceding count fails its income target.

| Illustrative operating target | $49 campaigns / recognized gross | $59 campaigns / recognized gross |
| --- | ---: | ---: |
| $10,000/month after illustrative tax | 949 / $46,501 | 664 / $39,176 |
| $50,000/month after illustrative tax | 3,262 / $159,838 | 2,281 / $134,579 |

These are not user targets, forecasts or demonstrated server capacity. Contribution
margins remain below70% at all these populations. At higher populations, $8,000 fixed
operations and support assumptions may fail; no linear infinite-scaling promise.
Six-month CAC payback can pass while the margin gate fails, and neither proves
company income. A separate cohort/effect-liability cash model is needed for growth.

## Proposed minimal plan refinement

No new family, crate, RPC or application code is justified. Extend existing
`X12-RESOLVE` and `X12-ACCEPT` through the current commercial-validation source:

1. Resolve a versioned offer qualification tuple: matched aggregate workload and
   features, measured billed generation/delivery/rights, loaded support cost,
   refunds/chargebacks/fees, chosen net-margin target, CAC/payback and fixed costs.
   Record joint support+generation budget and price/allowance counterfactuals.
2. Before a paid offer is called qualified, compare actual cohort mean joint costs
   with that budget, separating onboarding month from recurring months. Report
   distribution, sample size, cost correlation and uncertainty; p95 is an operational
   tail, not a substitute for cohort mean. Include denied/failed joins, human ruling
   assistance, refunds and billed failed attempts rather than successful sessions only.
3. If the proposed70% gate fails, retain an explicit FAIL with measured remediation
   or a product-owner-approved alternative margin/risk decision. No silent optimistic
   support reduction, customer-help denial, feature/rule/group shrink or credit profit
   used to mask the base subscription. Actual adoption and paid retention remain gates.

The current generic margin/CAC tests already exist. This refinement makes the joint
feasibility frontier and measured offer evidence explicit within their existing
ownership; it does not create a duplicate commercial-validation backlog.

## Evidence identity

Read current pushed main `aaf44f5bc93501b3fce4059d31dd17a10927c77e`.
Calculator SHA256 `408c68cdb11fc22ab34952257a187f590726579c034e776eb51c663c5f04b67a`;
inputs SHA256 `bc1f60f642205e3791ca54aa37009c2ab1593daffea79775169a750852faf1c1`.
No source assumption, provider facts or customer records were changed.
Independent numeric review PASS: all 30 grid rows, 10 table rows, four income minima
and 26 independently prepared boundary checks. The narrow existing-plan criteria
also passed independent review. This does not extend the historical 9.05/10 design
score into commercial proof.

Final actual integration PASS: source `11b95cdca8377b33867e9991df2d0029d27697d0ee677c99a5fb132796b1c21c`, manifest `5a8c0c6a952c04b8e700bc6d5235f166a785d1702ca6cf59334ad3b2e0b858d9`. No new tasks or edges; all original execution states and 74 prior devlogs preserved, with 87 final devlogs after the recorder and evaluator entries.
