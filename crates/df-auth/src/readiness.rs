//! Pure campaign-capacity and current-member readiness aggregation.
//!
//! The caller supplies the selected campaign capacity and the authoritative
//! session's current member IDs. This module does not own membership, persist
//! readiness, grant access, or choose a campaign capacity.

use std::collections::HashSet;

use df_types::MemberId;

/// Typed refusal when the supplied capacity or current readiness facts conflict.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ReadinessError {
    ZeroCapacity,
    CapacityExceeded,
    DuplicateCurrentMember,
    DuplicateReadyMember,
    ReadyMemberNotCurrent,
    MemberAlreadyPresent,
    CapacityFull,
}

/// Counts for one caller-supplied campaign capacity and current roster.
///
/// The current roster remains owned by the caller. This aggregate stores counts
/// only, so it cannot become another membership registry or authorization grant.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ReadinessAggregate {
    capacity: usize,
    occupied: usize,
    ready: usize,
}

impl ReadinessAggregate {
    /// The selected campaign capacity, without an assumed default or maximum.
    pub fn capacity(self) -> usize {
        self.capacity
    }

    /// Number of distinct current members in the supplied session snapshot.
    pub fn occupied(self) -> usize {
        self.occupied
    }

    /// Number of current members explicitly present in the ready-member slice.
    pub fn ready(self) -> usize {
        self.ready
    }

    /// Number of campaign-selected slots not occupied by current members.
    pub fn available_slots(self) -> usize {
        self.capacity - self.occupied
    }

    /// Whether the current roster leaves room for another distinct member.
    pub fn has_capacity(self) -> bool {
        self.available_slots() > 0
    }

    /// Whether a nonempty current roster is entirely ready.
    pub fn all_ready(self) -> bool {
        self.occupied > 0 && self.ready == self.occupied
    }
}

/// Aggregates readiness using the campaign's selected capacity and current IDs.
///
/// `current_members` must be the session owner's current authoritative roster;
/// `ready_members` contains only current members marked ready in that same
/// snapshot. The capacity must be positive and at least the roster size. This
/// function does not invent a two-seat limit, cap capacity to a fixed number, or
/// treat an empty roster as ready.
pub fn aggregate_current_readiness(
    campaign_capacity: usize,
    current_members: &[MemberId],
    ready_members: &[MemberId],
) -> Result<ReadinessAggregate, ReadinessError> {
    let current = checked_current_members(campaign_capacity, current_members)?;
    let mut ready = HashSet::with_capacity(ready_members.len());

    for member in ready_members {
        if !current.contains(member) {
            return Err(ReadinessError::ReadyMemberNotCurrent);
        }
        if !ready.insert(*member) {
            return Err(ReadinessError::DuplicateReadyMember);
        }
    }

    Ok(ReadinessAggregate {
        capacity: campaign_capacity,
        occupied: current_members.len(),
        ready: ready.len(),
    })
}

/// Checks whether a distinct canonical member can join the current roster.
///
/// This returns a decision only; it does not mutate membership or reserve a
/// slot. The session owner must serialize this check with its membership write.
pub fn check_member_admission(
    campaign_capacity: usize,
    current_members: &[MemberId],
    candidate: MemberId,
) -> Result<(), ReadinessError> {
    let current = checked_current_members(campaign_capacity, current_members)?;
    if current.contains(&candidate) {
        return Err(ReadinessError::MemberAlreadyPresent);
    }
    if current_members.len() == campaign_capacity {
        return Err(ReadinessError::CapacityFull);
    }
    Ok(())
}

fn checked_current_members(
    campaign_capacity: usize,
    current_members: &[MemberId],
) -> Result<HashSet<MemberId>, ReadinessError> {
    if campaign_capacity == 0 {
        return Err(ReadinessError::ZeroCapacity);
    }
    if current_members.len() > campaign_capacity {
        return Err(ReadinessError::CapacityExceeded);
    }

    let mut current = HashSet::with_capacity(current_members.len());
    for member in current_members {
        if !current.insert(*member) {
            return Err(ReadinessError::DuplicateCurrentMember);
        }
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::{ReadinessError, aggregate_current_readiness, check_member_admission};
    use df_types::MemberId;

    fn member(value: u8) -> MemberId {
        MemberId::from_bytes(&[value; 16]).expect("fixture identity is valid")
    }

    #[test]
    fn selected_capacity_above_two_counts_slots_and_admits_until_full() {
        let current = [member(1), member(2), member(3)];
        let aggregate = aggregate_current_readiness(4, &current, &[]).unwrap();

        assert_eq!(aggregate.capacity(), 4);
        assert_eq!(aggregate.occupied(), 3);
        assert_eq!(aggregate.ready(), 0);
        assert_eq!(aggregate.available_slots(), 1);
        assert!(aggregate.has_capacity());
        assert_eq!(check_member_admission(4, &current, member(4)), Ok(()));

        let full = [member(1), member(2), member(3), member(4)];
        let aggregate = aggregate_current_readiness(4, &full, &[]).unwrap();
        assert_eq!(aggregate.available_slots(), 0);
        assert!(!aggregate.has_capacity());
        assert_eq!(
            check_member_admission(4, &full, member(5)),
            Err(ReadinessError::CapacityFull)
        );
    }

    #[test]
    fn empty_roster_has_all_slots_available_but_is_not_ready() {
        let aggregate = aggregate_current_readiness(5, &[], &[]).unwrap();

        assert_eq!(aggregate.occupied(), 0);
        assert_eq!(aggregate.ready(), 0);
        assert_eq!(aggregate.available_slots(), 5);
        assert!(aggregate.has_capacity());
        assert!(!aggregate.all_ready());
    }

    #[test]
    fn readiness_counts_only_explicit_current_members() {
        let current = [member(1), member(2), member(3)];
        let ready = [member(1), member(3)];
        let aggregate = aggregate_current_readiness(3, &current, &ready).unwrap();

        assert_eq!(aggregate.occupied(), 3);
        assert_eq!(aggregate.ready(), 2);
        assert_eq!(aggregate.available_slots(), 0);
        assert!(!aggregate.all_ready());

        let aggregate = aggregate_current_readiness(3, &current, &current).unwrap();
        assert!(aggregate.all_ready());
    }

    #[test]
    fn invalid_capacity_and_rosters_are_typed_refusals() {
        assert_eq!(
            aggregate_current_readiness(0, &[], &[]),
            Err(ReadinessError::ZeroCapacity)
        );
        assert_eq!(
            aggregate_current_readiness(2, &[member(1), member(2), member(3)], &[]),
            Err(ReadinessError::CapacityExceeded)
        );
        assert_eq!(
            aggregate_current_readiness(2, &[member(1), member(1)], &[]),
            Err(ReadinessError::DuplicateCurrentMember)
        );
        assert_eq!(
            aggregate_current_readiness(2, &[member(1), member(2)], &[member(1), member(1)]),
            Err(ReadinessError::DuplicateReadyMember)
        );
        assert_eq!(
            aggregate_current_readiness(2, &[member(1)], &[member(2)]),
            Err(ReadinessError::ReadyMemberNotCurrent)
        );
        assert_eq!(
            check_member_admission(2, &[member(1)], member(1)),
            Err(ReadinessError::MemberAlreadyPresent)
        );
    }
}
