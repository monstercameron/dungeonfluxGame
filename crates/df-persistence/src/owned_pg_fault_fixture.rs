//! Exact bounded fixture fault SQL. No user-selected SQL identifier is interpolated.
use df_session::submission::RepositoryError;
use tokio::time::{Instant, timeout_at};
use tokio_postgres::Client;

#[derive(Clone, Copy)]
pub(crate) enum InsertedFamily {
    Checkpoint,
    Fact,
    Operation,
    Intent,
    FinalRevisionCas,
    LeaseBeforeCas,
    FactOrdinal,
    IntentSlot,
    EffectIdentity,
    CreatedJob,
    CreatedTimer,
}
impl InsertedFamily {
    fn install(self) -> &'static str {
        match self {
            Self::Checkpoint => {
                "CREATE TRIGGER fixture_fail_family AFTER INSERT ON df_game.checkpoints FOR EACH ROW EXECUTE FUNCTION df_fixture_authority.fail_inserted_family()"
            }
            Self::Fact => {
                "CREATE TRIGGER fixture_fail_family AFTER INSERT ON df_game.facts FOR EACH ROW EXECUTE FUNCTION df_fixture_authority.fail_inserted_family()"
            }
            Self::Operation => {
                "CREATE TRIGGER fixture_fail_family AFTER INSERT ON df_game.operations FOR EACH ROW EXECUTE FUNCTION df_fixture_authority.fail_inserted_family()"
            }
            Self::Intent => {
                "CREATE TRIGGER fixture_fail_family AFTER INSERT ON df_game.intents FOR EACH ROW EXECUTE FUNCTION df_fixture_authority.fail_inserted_family()"
            }
            Self::FinalRevisionCas => {
                "CREATE TRIGGER fixture_fail_final_cas BEFORE UPDATE OF in_epoch_sequence ON df_game.sessions FOR EACH ROW EXECUTE FUNCTION df_fixture_authority.refuse_final_revision_cas()"
            }
            Self::LeaseBeforeCas => {
                "CREATE TRIGGER fixture_lease_barrier AFTER INSERT ON df_game.intents FOR EACH ROW EXECUTE FUNCTION df_fixture_authority.expire_lease_before_cas()"
            }
            Self::FactOrdinal => {
                "CREATE TRIGGER fixture_unique_projection BEFORE INSERT ON df_game.facts FOR EACH ROW EXECUTE FUNCTION df_fixture_authority.collide_projection_identity('fact-ordinal')"
            }
            Self::IntentSlot => {
                "CREATE TRIGGER fixture_unique_projection BEFORE INSERT ON df_game.intents FOR EACH ROW EXECUTE FUNCTION df_fixture_authority.collide_projection_identity('intent-slot')"
            }
            Self::EffectIdentity => {
                "CREATE TRIGGER fixture_unique_projection BEFORE INSERT ON df_game.intents FOR EACH ROW EXECUTE FUNCTION df_fixture_authority.collide_projection_identity('effect-id')"
            }
            Self::CreatedJob => {
                "CREATE TRIGGER fixture_unique_projection BEFORE INSERT ON df_game.intents FOR EACH ROW EXECUTE FUNCTION df_fixture_authority.collide_projection_identity('created-job')"
            }
            Self::CreatedTimer => {
                "CREATE TRIGGER fixture_unique_projection BEFORE INSERT ON df_game.intents FOR EACH ROW EXECUTE FUNCTION df_fixture_authority.collide_projection_identity('created-timer')"
            }
        }
    }
    fn remove(self) -> &'static str {
        match self {
            Self::Checkpoint => "DROP TRIGGER fixture_fail_family ON df_game.checkpoints",
            Self::Fact => "DROP TRIGGER fixture_fail_family ON df_game.facts",
            Self::Operation => "DROP TRIGGER fixture_fail_family ON df_game.operations",
            Self::Intent => "DROP TRIGGER fixture_fail_family ON df_game.intents",
            Self::FinalRevisionCas => "DROP TRIGGER fixture_fail_final_cas ON df_game.sessions",
            Self::LeaseBeforeCas => "DROP TRIGGER fixture_lease_barrier ON df_game.intents",
            Self::FactOrdinal => "DROP TRIGGER fixture_unique_projection ON df_game.facts",
            Self::IntentSlot | Self::EffectIdentity | Self::CreatedJob | Self::CreatedTimer => {
                "DROP TRIGGER fixture_unique_projection ON df_game.intents"
            }
        }
    }
}
pub(crate) async fn install_fault(
    admin: &Client,
    family: InsertedFamily,
    deadline: Instant,
) -> Result<(), RepositoryError> {
    timeout_at(deadline, admin.batch_execute(family.install()))
        .await
        .map_err(|_| RepositoryError::Unavailable)?
        .map_err(|_| RepositoryError::Unavailable)
}
pub(crate) async fn remove_fault(
    admin: &Client,
    family: InsertedFamily,
    deadline: Instant,
) -> Result<(), RepositoryError> {
    timeout_at(deadline, admin.batch_execute(family.remove()))
        .await
        .map_err(|_| RepositoryError::Unavailable)?
        .map_err(|_| RepositoryError::Unavailable)
}
