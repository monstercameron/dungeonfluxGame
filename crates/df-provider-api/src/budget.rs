use df_model::checkpoint::{ExecutionMode, JobId};
use df_types::{Money, OperationId, Usage};

/// One candidate for the commerce authority's durable, hierarchical admission.
///
/// `Scope`, `Grant` and `Quote` are native owner-validated values. The quote must
/// bind source, configuration, provider, price, billable units and expiry; the
/// grant must express current customer consent. These references cannot establish
/// either fact by themselves. `maximum_supplier_liability` is an exact tagged
/// monetary maximum, not an estimated customer charge. A live candidate requires
/// a current entitlement, rights, consent and accepted intent at the native owner.
/// Prepared-only and replay work must not acquire a new paid reservation.
pub struct BudgetAdmission<'a, Scope, Grant, Quote> {
    pub scope: &'a Scope,
    pub grant: &'a Grant,
    pub quote: &'a Quote,
    pub job: JobId,
    pub operation: OperationId,
    pub mode: ExecutionMode,
    pub maximum_usage: Usage,
    pub maximum_supplier_liability: Money,
}

/// Outcome of one commerce-backed mutation, with no implied provider send.
///
/// `Committed` acknowledges a durable result; an exact retry returns its original
/// result with `replayed: true`. `Refused` and `Failed` prove no mutation was
/// committed. `Pending` has an identified accepted operation whose durable permit
/// is not ready. `Unknown` means a commit or external send may have occurred:
/// retain the full worst-case liability and reconcile by the same identity before
/// any retry. Neither cancellation nor expiry converts `Unknown` to `Failed`.
pub enum BudgetMutation<Result, Refusal, Failure> {
    Committed { result: Result, replayed: bool },
    Refused(Refusal),
    Pending,
    Unknown,
    Failed(Failure),
}

/// Consumer-owned port to the single commerce ledger, implemented by native
/// composition/persistence. It owns no counters, wallet, policy or provider socket.
///
/// `reserve` must revalidate the current trusted scope, entitlement, grant, quote,
/// mode and all hierarchical money/concurrency/rate limits in one durable admission
/// with the accepted intent. `claim_dispatch` must recheck its reservation, attempt,
/// owner fence and protected journal before returning a permit; only the native
/// egress dispatcher may use that permit. A lost acknowledgement is `Unknown`, not
/// a new paid key. `settle` uses verified attempt-bound actual usage and billed
/// waste once; `reconcile` inspects the same reservation/attempt and never releases
/// an unresolved liability on lease loss or timeout. Implementations use the
/// canonical df-commerce policy and ledger; these signatures do not implement it.
///
/// Opaque associated types are supplied by the auth, commerce and native ledger
/// owners. In particular, a `DispatchPermit` value alone is not send authority.
/// Calls have bounded native deadlines; loss of a caller wait does not cancel an
/// accepted reservation or an ambiguous provider attempt.
pub trait BudgetStore {
    type Scope;
    type Grant;
    type Quote;
    type Reservation;
    type DispatchClaim;
    type DispatchPermit;
    type Settlement;
    type SettlementReceipt;
    type LedgerView;
    type Refusal;
    type Failure;

    /// Read-only current scoped cost view for an authorized consumer.
    fn inspect(&mut self, scope: &Self::Scope) -> Result<Self::LedgerView, Self::Failure>;

    /// Atomically accept or refuse one quoted, consented maximum across the ledger.
    fn reserve(
        &mut self,
        admission: BudgetAdmission<'_, Self::Scope, Self::Grant, Self::Quote>,
    ) -> BudgetMutation<Self::Reservation, Self::Refusal, Self::Failure>;

    /// Claim one attempt-bound, journal-confirmed permission for native egress.
    fn claim_dispatch(
        &mut self,
        reservation: &Self::Reservation,
        claim: &Self::DispatchClaim,
    ) -> BudgetMutation<Self::DispatchPermit, Self::Refusal, Self::Failure>;

    /// Record verified actual supplier liability and release only known unused funds.
    fn settle(
        &mut self,
        reservation: &Self::Reservation,
        settlement: &Self::Settlement,
    ) -> BudgetMutation<Self::SettlementReceipt, Self::Refusal, Self::Failure>;

    /// Resolve the same attempt after an ambiguous mutation or possible send.
    fn reconcile(
        &mut self,
        reservation: &Self::Reservation,
    ) -> BudgetMutation<Self::SettlementReceipt, Self::Refusal, Self::Failure>;
}
