use crate::{library::RepositoryError, network::CancellationToken};
use mediaatlas_domain::ProviderFailure;
use std::{collections::HashMap, sync::{Arc, Mutex, Weak}};
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};
use uuid::Uuid;

#[derive(Default)]
pub(crate) struct DetailRequests {
    pending: Mutex<HashMap<(Uuid, usize), Weak<AsyncMutex<()>>>>,
}

impl DetailRequests {
    pub(crate) async fn lock(
        &self,
        run_id: Uuid,
        index: usize,
        cancel: &CancellationToken,
    ) -> Result<OwnedMutexGuard<()>, RepositoryError> {
        let gate = {
            let mut pending = self.pending.lock().map_err(|_| RepositoryError::Database)?;
            // Retain only active requests and their waiters, not the session history.
            pending.retain(|_, gate| gate.strong_count() > 0);
            let entry = pending.entry((run_id, index)).or_default();
            if let Some(gate) = entry.upgrade() {
                gate
            } else {
                let gate = Arc::new(AsyncMutex::new(()));
                *entry = Arc::downgrade(&gate);
                gate
            }
        };
        tokio::select! {
            biased;
            _ = cancel.cancelled() => Err(RepositoryError::Provider(ProviderFailure::Cancelled)),
            guard = gate.lock_owned() => Ok(guard),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn waiting_cancellation_keeps_other_candidates_and_the_owner_independent() {
        let requests = DetailRequests::default();
        let run = Uuid::new_v4();
        let owner_cancel = CancellationToken::new();
        let owner = requests.lock(run, 0, &owner_cancel).await.unwrap();
        let waiter_cancel = CancellationToken::new();
        let waiter = requests.lock(run, 0, &waiter_cancel);
        tokio::pin!(waiter);
        tokio::select! {
            biased;
            _ = &mut waiter => panic!("same candidate must wait for its owner"),
            _ = tokio::task::yield_now() => {}
        }
        let other = tokio::time::timeout(
            Duration::from_secs(1), requests.lock(run, 1, &owner_cancel),
        ).await.unwrap().unwrap();
        waiter_cancel.cancel();
        assert!(matches!(waiter.await, Err(RepositoryError::Provider(ProviderFailure::Cancelled))));
        assert!(!owner_cancel.is_cancelled());
        drop(owner);
        drop(other);
        let next = requests.lock(Uuid::new_v4(), 0, &owner_cancel).await.unwrap();
        assert_eq!(requests.pending.lock().unwrap().len(), 1);
        drop(next);
    }
}
