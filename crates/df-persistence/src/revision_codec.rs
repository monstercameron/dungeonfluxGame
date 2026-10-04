use df_types::{RecoveryEpoch, SessionRevision};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RevisionDecodeError {
    InvalidNumeric,
    InvalidEpoch,
}

/// PostgreSQL NUMERIC text is mapped without signed BIGINT narrowing or permissive parsing.
pub(crate) fn decode_unsigned_number(value: &str) -> Result<u64, RevisionDecodeError> {
    if value.is_empty()
        || value.len() > 20
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(RevisionDecodeError::InvalidNumeric);
    }
    value
        .parse()
        .map_err(|_| RevisionDecodeError::InvalidNumeric)
}

pub(crate) fn decode_revision(
    epoch: &str,
    sequence: &str,
) -> Result<SessionRevision, RevisionDecodeError> {
    let epoch = RecoveryEpoch::new(decode_unsigned_number(epoch)?)
        .map_err(|_| RevisionDecodeError::InvalidEpoch)?;
    Ok(SessionRevision::new(
        epoch,
        decode_unsigned_number(sequence)?,
    ))
}

pub(crate) fn encode_revision(revision: SessionRevision) -> (String, String) {
    (
        revision.epoch().get().to_string(),
        revision.sequence().to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_full_u64_revision_round_trips() {
        for epoch in [1, i64::MAX as u64 + 1, u64::MAX] {
            for sequence in [0, i64::MAX as u64 + 1, u64::MAX] {
                let revision = SessionRevision::new(RecoveryEpoch::new(epoch).unwrap(), sequence);
                let (epoch_text, sequence_text) = encode_revision(revision);
                assert_eq!(decode_revision(&epoch_text, &sequence_text), Ok(revision));
            }
        }
    }

    #[test]
    fn malformed_or_out_of_range_database_numbers_refuse() {
        for value in [
            "",
            "-1",
            "+1",
            "01",
            "1.0",
            "1e2",
            " 1",
            "18446744073709551616",
        ] {
            assert_eq!(
                decode_revision(value, "0"),
                Err(RevisionDecodeError::InvalidNumeric)
            );
            assert_eq!(
                decode_revision("1", value),
                Err(RevisionDecodeError::InvalidNumeric)
            );
        }
        assert_eq!(
            decode_revision("0", "0"),
            Err(RevisionDecodeError::InvalidEpoch)
        );
    }

    #[test]
    fn epoch_rollover_does_not_narrow_or_compare_sequence_alone() {
        let previous = decode_revision("1", "18446744073709551615").unwrap();
        let current = decode_revision("2", "0").unwrap();
        assert!(current > previous);
    }
}
