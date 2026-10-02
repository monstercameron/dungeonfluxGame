use df_content::graph::{
    GraphAlternative, GraphBeat, GraphError, GraphLimit, GraphLimits, GraphView, validate_graph,
};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct BeatId(u32);
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FactId(u32);
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct AlternativeId(u32);

const ONE: BeatId = BeatId(1);
const TWO: BeatId = BeatId(2);
const THREE: BeatId = BeatId(3);
const FOUR: BeatId = BeatId(4);
const FIVE: BeatId = BeatId(5);
const SIX: BeatId = BeatId(6);
const ABSENT: BeatId = BeatId(99);
const FACT: FactId = FactId(7);
const ROUTE: AlternativeId = AlternativeId(10);
const LIMITS: GraphLimits = GraphLimits {
    max_beats: 6,
    max_edges: 4,
    max_facts: 1,
    max_alternatives: 2,
    max_work: 1_000,
};

type Refusal = GraphError<BeatId, FactId, AlternativeId>;

struct Fixture {
    beats: Vec<GraphBeat<'static, BeatId>>,
    facts: Vec<FactId>,
    alternatives: Vec<GraphAlternative<'static, BeatId, FactId, AlternativeId>>,
}

impl Fixture {
    fn branching() -> Self {
        Self {
            beats: vec![
                GraphBeat {
                    id: &ONE,
                    next: &[TWO, THREE],
                    terminal: false,
                },
                GraphBeat {
                    id: &TWO,
                    next: &[FOUR],
                    terminal: false,
                },
                GraphBeat {
                    id: &THREE,
                    next: &[FIVE],
                    terminal: false,
                },
                GraphBeat {
                    id: &FOUR,
                    next: &[],
                    terminal: true,
                },
                GraphBeat {
                    id: &FIVE,
                    next: &[],
                    terminal: true,
                },
                GraphBeat {
                    id: &SIX,
                    next: &[],
                    terminal: true,
                },
            ],
            facts: vec![FACT],
            alternatives: vec![
                GraphAlternative {
                    id: &ROUTE,
                    owner: &ONE,
                    entry: &TWO,
                    exit: &FOUR,
                    disclosure_fact: None,
                },
                GraphAlternative {
                    id: &AlternativeId(11),
                    owner: &ONE,
                    entry: &THREE,
                    exit: &FIVE,
                    disclosure_fact: Some(&FACT),
                },
            ],
        }
    }

    fn view(&self) -> GraphView<'_, BeatId, FactId, AlternativeId> {
        GraphView {
            start: &ONE,
            beats: &self.beats,
            facts: &self.facts,
            alternatives: &self.alternatives,
        }
    }

    fn validate(&self) -> Result<(), Refusal> {
        validate_graph(&self.view(), LIMITS)
    }
}

#[test]
fn accepts_branching_routes_with_distinct_caller_owned_ids_and_disclosure_timing() {
    let fixture = Fixture::branching();
    assert_eq!(fixture.validate(), Ok(()));
    assert_eq!(fixture.beats[0].next, &[TWO, THREE]);
    assert_eq!(fixture.alternatives[1].disclosure_fact, Some(&FACT));
}

#[test]
fn diagnoses_missing_start_and_exact_edge_without_repairing_input() {
    let mut fixture = Fixture::branching();
    let mut graph = fixture.view();
    graph.start = &ABSENT;
    assert_eq!(
        validate_graph(&graph, LIMITS),
        Err(Refusal::MissingStart(ABSENT))
    );
    fixture.beats[1].next = &[ABSENT];
    assert_eq!(
        fixture.validate(),
        Err(Refusal::MissingEdge {
            from: TWO,
            to: ABSENT
        })
    );
    assert_eq!(fixture.beats[1].next, &[ABSENT]);
}

#[test]
fn rejects_duplicate_beat_fact_and_route_identities() {
    let mut fixture = Fixture::branching();
    fixture.beats[5].id = &ONE;
    assert_eq!(fixture.validate(), Err(Refusal::DuplicateBeat(ONE)));
    let mut fixture = Fixture::branching();
    fixture.facts.push(FACT);
    assert_eq!(
        validate_graph(
            &fixture.view(),
            GraphLimits {
                max_facts: 2,
                ..LIMITS
            }
        ),
        Err(Refusal::DuplicateFact(FACT))
    );
    let mut fixture = Fixture::branching();
    fixture.alternatives[1].id = &ROUTE;
    assert_eq!(
        fixture.validate(),
        Err(Refusal::DuplicateAlternative(ROUTE))
    );
}

#[test]
fn rejects_self_multi_node_and_disconnected_cycles_with_back_edge_identity() {
    let mut fixture = Fixture::branching();
    fixture.beats[1].next = &[TWO];
    assert_eq!(
        fixture.validate(),
        Err(Refusal::Cycle { from: TWO, to: TWO })
    );
    fixture.beats[1].next = &[ONE];
    assert_eq!(
        fixture.validate(),
        Err(Refusal::Cycle { from: TWO, to: ONE })
    );
    let mut fixture = Fixture::branching();
    fixture.beats[5].next = &[SIX];
    assert_eq!(
        validate_graph(
            &fixture.view(),
            GraphLimits {
                max_edges: 5,
                ..LIMITS
            }
        ),
        Err(Refusal::Cycle { from: SIX, to: SIX })
    );
}

#[test]
fn requires_a_terminal_reachable_from_the_start() {
    let mut fixture = Fixture::branching();
    fixture.beats[3].terminal = false;
    fixture.beats[4].terminal = false;
    assert_eq!(fixture.validate(), Err(Refusal::NoReachableExit));
    // A terminal in a disconnected component does not establish a campaign exit.
    assert!(fixture.beats[5].terminal);
}

#[test]
fn diagnoses_every_missing_alternative_reference_and_disclosure_fact() {
    let mut fixture = Fixture::branching();
    fixture.alternatives[0].owner = &ABSENT;
    assert_eq!(
        fixture.validate(),
        Err(Refusal::MissingAlternativeOwner {
            alternative: ROUTE,
            owner: ABSENT
        })
    );
    fixture.alternatives[0].owner = &ONE;
    fixture.alternatives[0].entry = &ABSENT;
    assert_eq!(
        fixture.validate(),
        Err(Refusal::MissingAlternativeEntry {
            alternative: ROUTE,
            entry: ABSENT
        })
    );
    fixture.alternatives[0].entry = &TWO;
    fixture.alternatives[0].exit = &ABSENT;
    assert_eq!(
        fixture.validate(),
        Err(Refusal::MissingAlternativeExit {
            alternative: ROUTE,
            exit: ABSENT
        })
    );
    fixture.alternatives[0].exit = &FOUR;
    fixture.alternatives[0].disclosure_fact = Some(&FactId(99));
    assert_eq!(
        fixture.validate(),
        Err(Refusal::MissingDisclosureFact {
            alternative: ROUTE,
            fact: FactId(99)
        })
    );
}

#[test]
fn diagnoses_unreachable_owner_entry_exit_and_nonterminal_exit_separately() {
    let mut fixture = Fixture::branching();
    fixture.alternatives[0].owner = &SIX;
    assert_eq!(
        fixture.validate(),
        Err(Refusal::UnreachableAlternativeOwner {
            alternative: ROUTE,
            owner: SIX
        })
    );
    fixture.alternatives[0].owner = &TWO;
    fixture.alternatives[0].entry = &THREE;
    assert_eq!(
        fixture.validate(),
        Err(Refusal::UnreachableAlternativeEntry {
            alternative: ROUTE,
            owner: TWO,
            entry: THREE
        })
    );
    fixture.alternatives[0].owner = &ONE;
    fixture.alternatives[0].entry = &TWO;
    fixture.alternatives[0].exit = &FIVE;
    assert_eq!(
        fixture.validate(),
        Err(Refusal::UnreachableAlternativeExit {
            alternative: ROUTE,
            entry: TWO,
            exit: FIVE
        })
    );
    fixture.alternatives[0].exit = &THREE;
    assert_eq!(
        fixture.validate(),
        Err(Refusal::AlternativeExitNotTerminal {
            alternative: ROUTE,
            exit: THREE
        })
    );
}

#[test]
fn enforces_each_capacity_bound_before_validation_and_never_truncates_success() {
    let fixture = Fixture::branching();
    for (limits, limit) in [
        (
            GraphLimits {
                max_beats: 5,
                ..LIMITS
            },
            GraphLimit::Beats,
        ),
        (
            GraphLimits {
                max_edges: 3,
                ..LIMITS
            },
            GraphLimit::Edges,
        ),
        (
            GraphLimits {
                max_facts: 0,
                ..LIMITS
            },
            GraphLimit::Facts,
        ),
        (
            GraphLimits {
                max_alternatives: 1,
                ..LIMITS
            },
            GraphLimit::Alternatives,
        ),
        (
            GraphLimits {
                max_work: 0,
                ..LIMITS
            },
            GraphLimit::Work,
        ),
    ] {
        assert_eq!(
            validate_graph(&fixture.view(), limits),
            Err(Refusal::LimitExceeded(limit))
        );
    }
    assert_eq!(fixture.validate(), Ok(()));
}

#[test]
fn work_budget_bounds_traversal_and_exact_sufficient_budget_succeeds() {
    let fixture = Fixture::branching();
    let sufficient = (1..LIMITS.max_work)
        .find(|max_work| {
            validate_graph(
                &fixture.view(),
                GraphLimits {
                    max_work: *max_work,
                    ..LIMITS
                },
            )
            .is_ok()
        })
        .unwrap();
    assert!(sufficient > fixture.beats.len() + fixture.facts.len() + fixture.alternatives.len());
    assert_eq!(
        validate_graph(
            &fixture.view(),
            GraphLimits {
                max_work: sufficient - 1,
                ..LIMITS
            }
        ),
        Err(Refusal::LimitExceeded(GraphLimit::Work))
    );
    assert_eq!(
        validate_graph(
            &fixture.view(),
            GraphLimits {
                max_work: sufficient,
                ..LIMITS
            }
        ),
        Ok(())
    );
}

#[test]
fn deep_chain_uses_iterative_validation_without_native_stack_recursion() {
    let ids: Vec<BeatId> = (0..20_000).map(BeatId).collect();
    let edges: Vec<Vec<BeatId>> = ids
        .windows(2)
        .map(|pair| vec![pair[1].clone()])
        .chain(std::iter::once(vec![]))
        .collect();
    let beats: Vec<GraphBeat<'_, BeatId>> = ids
        .iter()
        .zip(&edges)
        .enumerate()
        .map(|(index, (id, next))| GraphBeat {
            id,
            next,
            terminal: index == ids.len() - 1,
        })
        .collect();
    let graph: GraphView<'_, BeatId, FactId, AlternativeId> = GraphView {
        start: &ids[0],
        beats: &beats,
        facts: &[],
        alternatives: &[],
    };
    let limits = GraphLimits {
        max_beats: 20_000,
        max_edges: 19_999,
        max_facts: 0,
        max_alternatives: 0,
        max_work: 250_000,
    };
    assert_eq!(validate_graph(&graph, limits), Ok(()));
}

#[test]
fn single_terminal_start_is_valid_and_empty_graph_has_no_invented_start() {
    let beats = [GraphBeat {
        id: &ONE,
        next: &[],
        terminal: true,
    }];
    let graph: GraphView<'_, BeatId, FactId, AlternativeId> = GraphView {
        start: &ONE,
        beats: &beats,
        facts: &[],
        alternatives: &[],
    };
    assert_eq!(validate_graph(&graph, LIMITS), Ok(()));
    let graph = GraphView {
        beats: &[],
        ..graph
    };
    assert_eq!(
        validate_graph(&graph, LIMITS),
        Err(Refusal::MissingStart(ONE))
    );
}

#[test]
fn shared_descendants_are_not_cycles_and_record_order_fixes_first_diagnostic() {
    let mut fixture = Fixture::branching();
    fixture.alternatives.clear();
    fixture.beats[2].next = &[FOUR];
    assert_eq!(fixture.validate(), Ok(()));
    fixture.beats[1].next = &[BeatId(98), ABSENT];
    fixture.beats[2].next = &[ABSENT];
    assert_eq!(
        validate_graph(
            &fixture.view(),
            GraphLimits {
                max_edges: 5,
                ..LIMITS
            }
        ),
        Err(Refusal::MissingEdge {
            from: TWO,
            to: BeatId(98)
        })
    );
}
