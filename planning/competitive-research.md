# DungeonFlux competitive research

Snapshot: **September 29, 2026, America/New_York** (September 30 UTC).
Public official product, pricing, support and release pages; prices retain their
published currency/billing unit. No accounts, purchases or gameplay tests were
performed. “Documented” below means a vendor statement, not independently proven
correctness, latency, privacy enforcement or customer adoption. DungeonFlux is
currently a planned product; competitor features below are advertised offerings.

**TableForge and Friends & Fables are the primary AI tabletop benchmarks.**
LoreKeeper's Party Mode and DungeonsDeep's voice/multiplayer make the competition
closer than the pasted comparison suggests. DungeonFlux's most useful hypothesis
is an easy same-room TV-plus-phone campaign with clear, fair outcomes and responsive
voice/presentation. Crate architecture and more generated video do not establish
customer advantage.

## Shortlist and customer economics

| Product / category | Public price and payment unit | Documented capability and relevant uncertainty |
| --- | --- | --- |
| **Friends & Fables**, AI tabletop | Free: 25 GM turns/day, 3 players. $19.95/$29.95/$39.95 per subscriber/month: 4/5/6 players, unlimited standard turns and campaign slots; friends join free; 100/300/600 bonus credits | Persistent worlds, tactical 5e combat, images and paid-tier TTS. Premium narration/images consume credits; unlimited standard turns does not mean unlimited premium output. No dedicated TV/controller contract verified. [Pricing](https://fables.gg/pricing), [premium credits update, Dec 2 2025](https://fables.gg/patch-notes/patch-notes-2548-credits-image-studio-scene-generation-and-premium). |
| **TableForge (`tableforge.gg`)**, AI tabletop | Free: 60 campaign turns, one campaign/two players. $14.99/$29.99/$39.99 per host/month: 2/3/5 active campaigns, 2/4/6 players, unlimited campaign turns, 180/360/600 voice-narration minutes | Maps, scene art, scene-matched music and adaptive narration are already advertised. Guests free; multiple campaigns share a subscription, not separate campaign charges. Audio allowance is narration output, not gameplay hours. [Product/pricing](https://tableforge.gg/). |
| **LoreKeeper (`lore-keeper.com`)**, AI GM, early beta | Free: 20 turns/day; €7.99/€9.99/€19.99 monthly: 30/60/unlimited turns per day, 25/60/150 credits and 3/10/30 monthly images. All plans advertise six-player groups | Shared-screen Party Mode with voice/microphone, tactical grid arena, persistent wiki, autonomous NPCs and mechanics before prose. Separate phone-controller/private-view behavior is unverified. [Product](https://lore-keeper.com/en), [limits/beta status](https://lore-keeper.com/en/features). |
| **DungeonsDeep**, newer AI tabletop | $9.95/$19.95/$39.95 per subscriber/month for 750/1,600/4,000 GM responses; 75 trial turns; five-player parties | Nonpaying friends consume the present party's turn allowance; paying players' balances can contribute. Maps, voice interaction, game engine and campaign state are described. Vendor hour estimates need workload normalization. [Pricing, updated Aug 5 2026](https://dungeonsdeep.ai/pricing), [service/turn rules](https://dungeonsdeep.ai/legal/terms). |
| **AI Dungeon**, collaborative fiction | Free; Journey $14.99, Legend $29.99, Mythic $49.99, Ultimate $99.99 per month, with model/context/memory and credit differences | Multiplayer join codes; host membership supplies premium model benefits. Useful story/price benchmark, but standard-2024 rules authority, tactical map and voice paths were not verified in these sources. [Memberships](https://help.aidungeon.com/memberships-benefits), [multiplayer](https://help.aidungeon.com/faq/do-you-support-multiplayer). |
| **D&D Beyond Maps**, human-DM tool | Free hosting/joining; Hero $2.99 and Master $5.99 monthly, cheaper annual equivalents. Books purchased separately | Official character/content ecosystem. Master enables content sharing (advertised up to 12 players across five campaigns), custom maps and homebrew monsters. TV/projector Spectator View hides fog/secret tokens; free players control tokens. [Subscriptions](https://www.dndbeyond.com/en/subscribe), [Maps guide](https://www.dndbeyond.com/posts/1816-the-official-d-d-vtt-navigating-maps-on-d-d-beyond). |
| **Foundry VTT**, human-DM tool | $50 perpetual core license; hosting/content can cost extra; players need no license | Strong rules/content and extensibility benchmark. License permits one player-accessible server in active use; “unlimited players” is a license term, not measured capacity. Official 2024 core-book content is documented; no built-in autonomous AI GM was verified here. [Purchase](https://foundryvtt.com/purchase/), [FAQ](https://foundryvtt.com/article/faq/), [2026 content showcase](https://foundryvtt.com/article/content-showcase-2026/). |
| **Roll20**, human-DM tool | Free; indexed official page currently shows annual Pro $99.99 ($8.33 monthly equivalent), regular $109.99, and Elite $149.99, regular $159.99 | Hosting, maps/dynamic lighting, character sheets, content sharing and mods. Exact current monthly/Plus checkout rates were not reliably retrieved; do not recycle the 2021 price announcement as a 2026 quote. Sharing caps are not overall game-capacity limits. [Subscription page](https://app.roll20.net/why-subscribe-to-roll20), [2024 D&D support](https://pages.roll20.net/dnd2024). |
| **Jackbox Party Pack 11**, adjacent party experience | Official Steam-code store: $29.99 one-time; one owner hosts, other players use browser controllers | Proven design reference for TV/shared screen plus phone/tablet/laptop input; not a persistent AI D&D campaign. Game-specific player limits apply. [Store](https://checkout.jackboxgames.com/products/the-jackbox-party-pack-11), [setup guide](https://support.jackboxgames.com/hc/en-us/articles/15794771245975-How-do-I-get-started-playing-Jackbox-Games). |
| **LoreKeeper (`lorekeeper.com`)**, adjacent prep/media toolkit | About page: $10/$20/$50 monthly plus credits; free-credit amounts conflict between home/about pages, so exact signup offer is unverified | Different product/team from the hyphenated AI GM and `lorekeeper.ai`. Already advertises storyboards/short films, images, SFX, 3D models, and voiced Player Mode with push-to-talk. This weakens any “nobody does cinematic assets” claim. [About/disambiguation](https://www.lorekeeper.com/about), [toolkit](https://www.lorekeeper.com/). |

## Corrections to the pasted comparison

**Programmatic rules are an existing competitor claim.** TableForge explicitly
separates code-driven dice/combat/conditions/spell slots from narration. Its FAQ
calls the ruleset “SRD 2024,” while attribution includes SRD 5.1, SRD 5.2 and
Advanced 5th Edition material. That is a scope/mixing question to test, not proof
of flawless full 2024 core-book support. Campaign privacy in that FAQ means access
by campaign participants; it does not establish separate secret knowledge between
players or noninterference in generated sound. [Official FAQ](https://tableforge.gg/faq).

**Friends & Fables is not merely a chat loop.** Its Oct 30, 2025 engineering article
describes multiple LLM postprocessing/state-update paths. The Aug 1, 2025 Combat
V4 release describes agentic tools and acknowledges potential errors; this is a
vendor architecture description, not audited source. The current home page's SRD
5.1 attribution does not verify comprehensive 2024 support. Its “100,000+ players
and world builders” is a self-reported total, not active subscribers or revenue.
[How Franz works](https://fables.gg/blog/how-franz-works),
[Combat V4](https://fables.gg/patch-notes/combat-v4), [home page](https://fables.gg/).

**LoreKeeper needs an exact domain.** The hyphenated AI GM advertises deterministic
pre-narration calculations and authored/custom systems. Its competitor comparison
also claims 5e enforcement and says invited players use their own daily turns;
that is not the same allowance contract as a host covering unlimited guests. Exact
2024 source completeness and billing depletion behavior require validation. Do
not import claims from the separate prep toolkit or campaign manager.
[AI-GM vendor comparison](https://lore-keeper.com/blog/lorekeeper-vs-friends-and-fables),
[worldbuilder](https://lore-keeper.com/en/for-worldbuilders).

**“Live audiovisual direction” cannot be marked absent.** TableForge documents
adaptive tone and music; LoreKeeper documents voice/Party Mode; the other
LoreKeeper markets short-film tools. None of the reviewed public material proves
an equivalent continuous multi-channel tempo controller or budgeted predictive
asset scheduler, but missing documentation is **unverified**, not “doesn't exist.”
A competitor's generic cinematic wording also does not prove those implementations.
[TableForge operation](https://tableforge.gg/how-it-works),
[LoreKeeper toolkit](https://www.lorekeeper.com/).

The original `:chatgpt-content-reference{...}` tokens are not usable citations.
This report replaces them with official links. Product comparison pages are
self-interested: use a vendor's description of its own product, not its claims
that every competitor is inferior. Source attribution or advertised 5e support
alone does not establish licensed full-book access, tested rule coverage or legal
rights to reproduce every D&D publication. DungeonFlux's exact source/catalog
and rights gate remains open.

## Pricing implication for DungeonFlux

The planned **$49–$59 per active campaign/month plus optional video credits** faces
host subscriptions that include several campaigns and common group sizes around
$20–$40. The meaningful comparison is the same group's usage, seats, narration,
images and simultaneous campaigns—not the price label alone. A $49 campaign is
about 23% above TableForge's $39.99 six-player host plan; $59 is about 48% above.
For two active DungeonFlux campaigns, $98–$118 would compete with a $39.99
subscription advertising five campaign slots. This arithmetic does not establish
similar quality, workload or profitability. [TableForge pricing](https://tableforge.gg/).

Retain the cost model as a hypothesis; do not lower price below viable measured
COGS just to match a cheap tier. Test campaign versus host-bundle versus session
pricing before freezing allowances. Distinguish narration output minutes,
aggregate submitted speech, GM responses and player-hours. DungeonsDeep explicitly
multiplies table hours into player-hours in its marketing; 50 player-hours for five
players is approximately ten shared table hours, not fifty hours of simultaneous
five-person play. [DungeonsDeep pricing explanation](https://dungeonsdeep.ai/pricing).

Video remains an optional quoted purchase. Reusable stills, stable character art,
layered prepared music/SFX and restrained tempo can demonstrate value before
expensive cinema. The current core-cost estimate has not measured willingness to
pay, retention or customer support. A Rust/crate/perfect-logging story is valuable
engineering but customers buy a better game night. The counterargument is strong:
competitors may already deliver satisfactory fairness, voice, memory, private
information and identity consistency at a lower price. DungeonFlux must show the
benefit in play rather than assume it from planned architecture.

## Focused wedge and falsifiable tests

Proposed wedge: a no-human-DM, same-room session with a clean shared TV scene,
quick phone join/private actions, responsive validated voice, understandable
source-backed outcomes, persistent coherent NPC knowledge, and subtle accessible
audio/visual tempo. Use prepared assets/stills first. These are priorities for
existing plans, not newly authorized features or proof of product-market fit.

1. **Same-room setup and attention.** Give the same four-to-six-person groups an
   identical 30-minute scenario in TableForge, Friends & Fables, LoreKeeper Party
   Mode and a small DungeonFlux two-role prototype when ready. Measure time to
   first valid action, assistance required, disconnected-player recovery and
   minutes spent reading phones instead of interacting. Proposed pass hypothesis:
   most groups independently prefer DungeonFlux's setup/attention flow; report
   exact counts, not a promotional winner. Repeat with larger supported groups
   separately; “as many as a campaign supports” needs evidence.
2. **Fairness and private knowledge.** Prepare 20 source-linked cases covering
   scarce spell slots, reaction timing, concentration, stale/double input, NPC
   ignorance, private clue and hidden boss. Record actual product versions,
   explained outcomes, source correctness and visible/audio leaks. Invite expert
   blinded review where feasible. One leakage or fabricated success is an
   actionable defect; a small sample cannot prove exhaustive rules correctness.
3. **Voice and tempo ablation.** Compare prepared narration/still assets with and
   without layered tempo on the same scenario. Record final-input-to-first-approved
   audible output and p50/p95 latency, interruptions, caption match, input stalls,
   reduced-motion behavior and actual COGS. Test explicit barge-in, microphone
   denial and sleeping phones. Reject the tempo investment if preference/attention
   fails to improve or latency/fatigue worsens. No video is required for this test.
4. **Price and renewal.** Present qualified groups with concrete equal-usage offers
   at $29/$39/$49/$59, including campaign versus host-bundle terms, exact allowance
   and optional video quotes. Observe a voluntary pilot purchase/return commitment
   and four-week reuse, not just “would you pay?” answers. This research has not
   contacted customers or collected payments. Set viable cost/margin/retention
   thresholds before interpreting results; adverse demand revises the offer.
5. **Media value and waste.** Compare cached still/voice/tempo against optional
   short video and limited predictive prefetch under a fixed spend cap. Measure
   asset identity errors, blocked-play time, hit rate, discarded paid outputs,
   p95 session cost and voluntary willingness to buy more. Stop if paid waste or
   impatience offsets the quality preference. Future-branch assets must not leak
   secrets or imply that a predicted outcome already happened.

## Evidence limits and task proposals

No functional competitor benchmark was run. Source pages sometimes failed direct
fetch despite available official search-index text: Friends & Fables pricing and
engineering article, TableForge how-it-works, Roll20 subscription page and several
LoreKeeper pages. Such entries are marked in the comparison JSON; rates should be
rechecked in an actual regional checkout before commercial decisions. LoreKeeper
feature-table icons were absent in extracted text, so tier checkmarks were not
invented. “Unlimited” describes a published entitlement, not proven concurrency
or absence of fair-use restrictions. All prices exclude unverified taxes/FX.

Coordinator intake proposals, dependent on existing implementation gates:

- **COMP-VERIFY:** verify exact guest/host depletion, current checkout and 2024
  catalog scope for the three strongest AI tabletop competitors; retain dated
  screenshots/terms, no paid actions without their authorized test budget.
- **COMP-PLAYTEST:** execute tests 1–3 against a fixed DungeonFlux build and the
  comparable competitor revision; retain consented browser/audio evidence.
- **COMP-OFFER:** run the price/retention and media-value experiments after COGS
  measurement; compare per-campaign versus host-bundle economics before changing
  the price catalog or promising video allowances.

These are suggested resolution/evaluation tasks for the coordinator's SQLite
intake. This research did not edit queue state, source contracts or the architecture.
