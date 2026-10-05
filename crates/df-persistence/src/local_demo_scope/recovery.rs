//! The serialization owner retains every original/replacement grant connection.
use super::LocalDemoScopeIssuer;
use df_session::submission::RepositoryError;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio_postgres::{Config, Error, NoTls};

const CONNECTION_DEADLINE: Duration = Duration::from_secs(2);
type Driver = JoinHandle<Result<(), Error>>;

impl LocalDemoScopeIssuer {
    /// Transfer the initial grant driver and trusted native connection configuration.
    /// Existing constructor callers retain external ownership until this opt-in.
    /// Duplicate configuration refuses and returns the unadopted driver for joining.
    /// Reconnection never reuses cached grants or replays a transaction.
    pub fn configure_reconnect(
        &mut self,
        configuration: Config,
        driver: Driver,
    ) -> Result<(), (RepositoryError, Driver)> {
        if self.reconnect.is_some() || self.driver.is_some() {
            return Err((RepositoryError::InputBinding, driver));
        }
        self.reconnect = Some(configuration);
        self.driver = Some(driver);
        Ok(())
    }

    /// Called only by the joined serialization actor. A failed attempt leaves all
    /// driver ownership here and the canonical pending operation unchanged.
    pub fn reconnect_if_closed(&mut self) -> Result<(), RepositoryError> {
        let runtime = self.authority.runtime.clone();
        crate::native_connection::actor_block_on(&runtime, self.reconnect_owned())?
    }

    pub(super) async fn reconnect_owned(&mut self) -> Result<(), RepositoryError> {
        if self
            .authority
            .client
            .as_ref()
            .is_some_and(|client| !client.is_closed())
        {
            return Ok(());
        }
        let configuration = self.reconnect.clone().ok_or(RepositoryError::Unavailable)?;
        self.close_owned().await?;
        let (client, connection) =
            tokio::time::timeout(CONNECTION_DEADLINE, configuration.connect(NoTls))
                .await
                .map_err(|_| RepositoryError::Unavailable)?
                .map_err(|_| RepositoryError::Unavailable)?;
        self.driver = Some(self.authority.runtime.spawn(connection));
        self.authority.client = Some(client);
        Ok(())
    }

    /// Stop admission/drain first, release the exact owner fence, then close before
    /// joining the actor. A timed-out join retains the handle for another close attempt.
    pub fn close(&mut self) -> Result<(), RepositoryError> {
        let runtime = self.authority.runtime.clone();
        crate::native_connection::actor_block_on(&runtime, self.close_owned())?
    }

    /// Native setup/shutdown before actor handoff. Drop the client before joining;
    /// old protocol failure is an expected closed connection, not unjoined work.
    pub async fn close_owned(&mut self) -> Result<(), RepositoryError> {
        drop(self.authority.client.take());
        let Some(driver) = self.driver.as_mut() else {
            return Ok(());
        };
        let joined = match tokio::time::timeout(CONNECTION_DEADLINE, &mut *driver).await {
            Ok(result) => result.map(|_| ()).map_err(|_| RepositoryError::Unavailable),
            Err(_) => {
                driver.abort();
                match tokio::time::timeout(CONNECTION_DEADLINE, &mut *driver).await {
                    Ok(Ok(_)) => Ok(()),
                    Ok(Err(error)) if error.is_cancelled() => Ok(()),
                    Ok(Err(_)) => Err(RepositoryError::Unavailable),
                    Err(_) => return Err(RepositoryError::Unavailable),
                }
            }
        };
        self.driver.take();
        joined
    }
}

impl Drop for LocalDemoScopeIssuer {
    fn drop(&mut self) {
        // Emergency abandonment cannot detach a replacement driver. Ordinary
        // completion must observe close_owned/close; abort alone is not a join.
        drop(self.authority.client.take());
        if let Some(driver) = &self.driver {
            driver.abort();
        }
    }
}
