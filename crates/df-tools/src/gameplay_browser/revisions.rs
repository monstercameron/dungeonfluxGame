use df_protocol::common::SessionRevision;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct IncompleteRevision;

fn components(revision: SessionRevision) -> Result<(u64, u64), IncompleteRevision> {
    Ok((
        revision
            .epoch
            .and_then(|epoch| epoch.value)
            .ok_or(IncompleteRevision)?,
        revision.sequence.ok_or(IncompleteRevision)?,
    ))
}

/// Recovery epochs order snapshots before their in-epoch sequence. Equal revisions
/// remain renderable so local receipt/busy updates can redraw the retained view.
pub(super) fn accept_revision(
    previous: Option<SessionRevision>,
    next: SessionRevision,
) -> Result<bool, IncompleteRevision> {
    let next = components(next)?;
    match previous {
        Some(previous) => Ok(next >= components(previous)?),
        None => Ok(true),
    }
}
