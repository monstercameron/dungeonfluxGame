use df_types::{Currency, Money, MoneyError, OperationId};

/// Canonical counter order for caller-bound current periods in one currency.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpendScope {
    PlatformDay,
    PlatformMonth,
    SupplierAccount,
    TenantPayer,
    Campaign,
    Job,
}

const SPEND_SCOPES: [SpendScope; 6] = [
    SpendScope::PlatformDay,
    SpendScope::PlatformMonth,
    SpendScope::SupplierAccount,
    SpendScope::TenantPayer,
    SpendScope::Campaign,
    SpendScope::Job,
];

/// Exact current exposure, including every reserved and Unknown worst-case maximum.
/// Expiry, cancellation, lease loss or restore cannot subtract Unknown exposure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpendCounter {
    pub used: Money,
    pub limit: Money,
}

/// Supplied transaction snapshot, not a ledger or independent budget authority.
/// Rows are ordered platform day, platform month, supplier/account, tenant/payer,
/// campaign, job. Both platform period caps are mandatory, with no time-based reset.
/// The owner binds all rows to authenticated scope and locks them in this order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpendSnapshot {
    pub currency: Currency,
    pub revision: u64,
    pub counters: [SpendCounter; 6],
}

/// Supplied current consent in exact micro-units; no price or grant is selected here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpendConsent {
    pub revision: u64,
    pub maximum: Money,
}

/// Durable lookup status, supplied for the exact requested operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpendOperationStatus {
    Unseen,
    Recorded,
    Unknown,
}

/// Unknown never becomes Unseen merely because of expiry, cancellation or restore.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpendOperationObservation {
    pub operation: OperationId,
    pub status: SpendOperationStatus,
}

/// The quote owner supplies a conservatively computed maximum, never an estimate.
/// Native admission separately binds quote/price/source/provider/attempt and consent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpendRequest {
    pub operation: OperationId,
    pub expected_revision: u64,
    pub expected_consent_revision: u64,
    pub maximum_supplier_liability: Money,
}

/// Refusal produces no reservation, counter changes, customer charge or dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpendRefusal {
    StaleRevision,
    StaleConsent,
    WrongOperationObservation,
    DuplicateOperation,
    UnknownOperation,
    Money(MoneyError),
    CustomerConsentExceeded,
    BudgetExceeded(SpendScope),
    RevisionOverflow,
}

impl From<MoneyError> for SpendRefusal {
    fn from(error: MoneyError) -> Self {
        Self::Money(error)
    }
}

/// Candidate only, never a reservation, dispatch permit, charge or commit receipt.
/// Its owner rechecks revisions and uniqueness and atomically commits every counter
/// with reservation/accepted-intent linkage. Protected journal confirmation precedes
/// egress. Rights, entitlement, expiry, units, concurrency and storage remain required.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpendProposal {
    pub operation: OperationId,
    pub before_revision: u64,
    pub consent_revision: u64,
    pub consented_maximum: Money,
    pub maximum_supplier_liability: Money,
    pub next: SpendSnapshot,
}

/// Computes all six exposure increases in stable order without mutating input.
/// Money tags are checked before any arithmetic; no currency conversion is attempted.
/// Unknown operation liability cannot be resent or released through this function.
/// Shared exact `LiabilityRate::liability` can compute a rounded-up quoted maximum;
/// default rates, customer prices and invoice settlement are outside this boundary.
pub fn propose_spend(
    snapshot: &SpendSnapshot,
    consent: SpendConsent,
    request: SpendRequest,
    observation: SpendOperationObservation,
) -> Result<SpendProposal, SpendRefusal> {
    if request.expected_revision != snapshot.revision {
        return Err(SpendRefusal::StaleRevision);
    }
    if request.expected_consent_revision != consent.revision {
        return Err(SpendRefusal::StaleConsent);
    }
    if observation.operation != request.operation {
        return Err(SpendRefusal::WrongOperationObservation);
    }
    match observation.status {
        SpendOperationStatus::Recorded => return Err(SpendRefusal::DuplicateOperation),
        SpendOperationStatus::Unknown => return Err(SpendRefusal::UnknownOperation),
        SpendOperationStatus::Unseen => {}
    }
    let maximum = request.maximum_supplier_liability;
    if maximum.currency() != snapshot.currency
        || consent.maximum.currency() != snapshot.currency
        || snapshot.counters.iter().any(|counter| {
            counter.used.currency() != snapshot.currency
                || counter.limit.currency() != snapshot.currency
        })
    {
        return Err(MoneyError::CurrencyMismatch.into());
    }
    if maximum.micros() > consent.maximum.micros() {
        return Err(SpendRefusal::CustomerConsentExceeded);
    }

    let mut next = *snapshot;
    next.revision = snapshot
        .revision
        .checked_add(1)
        .ok_or(SpendRefusal::RevisionOverflow)?;
    for (scope, counter) in SPEND_SCOPES.into_iter().zip(next.counters.iter_mut()) {
        let proposed_used = counter.used.checked_add(maximum)?;
        if proposed_used.micros() > counter.limit.micros() {
            return Err(SpendRefusal::BudgetExceeded(scope));
        }
        counter.used = proposed_used;
    }
    Ok(SpendProposal {
        operation: request.operation,
        before_revision: snapshot.revision,
        consent_revision: consent.revision,
        consented_maximum: consent.maximum,
        maximum_supplier_liability: maximum,
        next,
    })
}
