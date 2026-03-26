use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use tokio::{sync::oneshot, time::timeout};

use crate::SynapseError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionSchedulerConfig {
    pub max_concurrent_executions: usize,
    pub max_queue_depth: usize,
    pub max_queue_timeout_ms: u64,
    pub max_concurrent_executions_per_tenant: usize,
}

impl ExecutionSchedulerConfig {
    pub fn new(
        max_concurrent_executions: usize,
        max_queue_depth: usize,
        max_queue_timeout_ms: u64,
        max_concurrent_executions_per_tenant: usize,
    ) -> Self {
        Self {
            max_concurrent_executions: max_concurrent_executions.max(1),
            max_queue_depth,
            max_queue_timeout_ms: max_queue_timeout_ms.max(1),
            max_concurrent_executions_per_tenant: max_concurrent_executions_per_tenant.max(1),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExecutionScheduler {
    config: ExecutionSchedulerConfig,
    inner: Arc<SchedulerInner>,
}

#[derive(Debug)]
struct SchedulerInner {
    state: Mutex<SchedulerState>,
    next_waiter_id: AtomicU64,
}

#[derive(Debug, Default)]
struct SchedulerState {
    active_total: usize,
    queued_total: usize,
    active_per_tenant: HashMap<String, usize>,
    tenant_order: VecDeque<String>,
    tenant_queues: HashMap<String, VecDeque<QueuedWaiter>>,
    admitted_total: u64,
    rejected_total: u64,
    queue_timeout_total: u64,
    queue_wait_time_ms_total: u64,
}

#[derive(Debug)]
struct QueuedWaiter {
    id: u64,
    enqueued_at: Instant,
    sender: oneshot::Sender<()>,
}

#[derive(Debug)]
pub struct ExecutionPermit {
    tenant_id: String,
    wait_duration_ms: u64,
    was_queued: bool,
    config: ExecutionSchedulerConfig,
    inner: Arc<SchedulerInner>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedulerMetrics {
    pub active_total: usize,
    pub queued_total: usize,
    pub admitted_total: u64,
    pub rejected_total: u64,
    pub queue_timeout_total: u64,
    pub queue_wait_time_ms_total: u64,
}

impl ExecutionScheduler {
    pub fn new(config: ExecutionSchedulerConfig) -> Self {
        Self {
            config,
            inner: Arc::new(SchedulerInner {
                state: Mutex::new(SchedulerState::default()),
                next_waiter_id: AtomicU64::new(1),
            }),
        }
    }

    pub fn config(&self) -> ExecutionSchedulerConfig {
        self.config
    }

    pub fn metrics(&self) -> SchedulerMetrics {
        let state = self.inner.state.lock().expect("scheduler mutex poisoned");
        SchedulerMetrics {
            active_total: state.active_total,
            queued_total: state.queued_total,
            admitted_total: state.admitted_total,
            rejected_total: state.rejected_total,
            queue_timeout_total: state.queue_timeout_total,
            queue_wait_time_ms_total: state.queue_wait_time_ms_total,
        }
    }

    pub async fn acquire(&self, tenant_id: &str) -> Result<ExecutionPermit, SynapseError> {
        let (waiter_id, receiver) = {
            let mut state = self.inner.state.lock().expect("scheduler mutex poisoned");
            if state.try_acquire_immediately(&self.config, tenant_id) {
                return Ok(ExecutionPermit {
                    tenant_id: tenant_id.to_string(),
                    wait_duration_ms: 0,
                    was_queued: false,
                    config: self.config,
                    inner: Arc::clone(&self.inner),
                });
            }

            if state.queued_total >= self.config.max_queue_depth {
                state.rejected_total += 1;
                return Err(SynapseError::CapacityRejected(format!(
                    "execution queue is full (depth {}, max {})",
                    state.queued_total, self.config.max_queue_depth
                )));
            }

            let waiter_id = self.inner.next_waiter_id.fetch_add(1, Ordering::Relaxed);
            let (sender, receiver) = oneshot::channel();
            state.enqueue(tenant_id, waiter_id, sender);
            (waiter_id, receiver)
        };

        let started = Instant::now();
        let timeout_duration = Duration::from_millis(self.config.max_queue_timeout_ms);
        let mut receiver = receiver;
        match timeout(timeout_duration, &mut receiver).await {
            Ok(Ok(())) => Ok(ExecutionPermit {
                tenant_id: tenant_id.to_string(),
                wait_duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                was_queued: true,
                config: self.config,
                inner: Arc::clone(&self.inner),
            }),
            Ok(Err(_)) => Err(SynapseError::Internal(
                "execution scheduler waiter dropped unexpectedly".to_string(),
            )),
            Err(_) => {
                let removed = {
                    let mut state = self.inner.state.lock().expect("scheduler mutex poisoned");
                    if state.remove_waiter(tenant_id, waiter_id) {
                        state.queue_timeout_total += 1;
                        true
                    } else {
                        false
                    }
                };

                if removed {
                    return Err(SynapseError::QueueTimeout(format!(
                        "execution waited longer than {} ms in queue",
                        self.config.max_queue_timeout_ms
                    )));
                }

                match receiver.await {
                    Ok(()) => Ok(ExecutionPermit {
                        tenant_id: tenant_id.to_string(),
                        wait_duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX))
                            as u64,
                        was_queued: true,
                        config: self.config,
                        inner: Arc::clone(&self.inner),
                    }),
                    Err(_) => Err(SynapseError::Internal(
                        "execution scheduler waiter dropped after queue race".to_string(),
                    )),
                }
            }
        }
    }
}

impl ExecutionPermit {
    pub fn was_queued(&self) -> bool {
        self.was_queued
    }

    pub fn wait_duration_ms(&self) -> u64 {
        self.wait_duration_ms
    }
}

impl Drop for ExecutionPermit {
    fn drop(&mut self) {
        let mut state = self.inner.state.lock().expect("scheduler mutex poisoned");
        state.release(&self.tenant_id, &self.config);
    }
}

impl SchedulerState {
    fn try_acquire_immediately(
        &mut self,
        config: &ExecutionSchedulerConfig,
        tenant_id: &str,
    ) -> bool {
        if self.queued_total > 0 && self.has_dispatchable_waiter(config) {
            return false;
        }
        if self.active_total >= config.max_concurrent_executions {
            return false;
        }
        if self.active_for_tenant(tenant_id) >= config.max_concurrent_executions_per_tenant {
            return false;
        }

        self.active_total += 1;
        *self
            .active_per_tenant
            .entry(tenant_id.to_string())
            .or_default() += 1;
        self.admitted_total += 1;
        true
    }

    fn has_dispatchable_waiter(&self, config: &ExecutionSchedulerConfig) -> bool {
        self.tenant_order.iter().any(|tenant_id| {
            self.tenant_queues.get(tenant_id).is_some_and(|queue| {
                !queue.is_empty()
                    && self.active_for_tenant(tenant_id)
                        < config.max_concurrent_executions_per_tenant
            })
        })
    }

    fn enqueue(&mut self, tenant_id: &str, id: u64, sender: oneshot::Sender<()>) {
        let queue = self.tenant_queues.entry(tenant_id.to_string()).or_default();
        if queue.is_empty() {
            self.tenant_order.push_back(tenant_id.to_string());
        }
        queue.push_back(QueuedWaiter {
            id,
            enqueued_at: Instant::now(),
            sender,
        });
        self.queued_total += 1;
    }

    fn remove_waiter(&mut self, tenant_id: &str, waiter_id: u64) -> bool {
        let Some(queue) = self.tenant_queues.get_mut(tenant_id) else {
            return false;
        };
        let Some(position) = queue.iter().position(|waiter| waiter.id == waiter_id) else {
            return false;
        };
        queue.remove(position);
        self.queued_total = self.queued_total.saturating_sub(1);
        if queue.is_empty() {
            self.tenant_queues.remove(tenant_id);
            self.tenant_order
                .retain(|queued_tenant| queued_tenant != tenant_id);
        }
        true
    }

    fn release(&mut self, tenant_id: &str, config: &ExecutionSchedulerConfig) {
        self.active_total = self.active_total.saturating_sub(1);
        if let Some(active) = self.active_per_tenant.get_mut(tenant_id) {
            *active = active.saturating_sub(1);
            if *active == 0 {
                self.active_per_tenant.remove(tenant_id);
            }
        }
        if self.tenant_order.len() > 1
            && matches!(self.tenant_order.front(), Some(front) if front == tenant_id)
        {
            let front = self.tenant_order.pop_front().expect("front checked above");
            self.tenant_order.push_back(front);
        }
        self.dispatch_next_available(config);
    }

    fn dispatch_next_available(&mut self, config: &ExecutionSchedulerConfig) {
        if self.active_total >= config.max_concurrent_executions {
            return;
        }

        let tenant_count = self.tenant_order.len();
        for _ in 0..tenant_count {
            let Some(tenant_id) = self.tenant_order.pop_front() else {
                return;
            };
            if self.active_for_tenant(&tenant_id) >= config.max_concurrent_executions_per_tenant {
                self.tenant_order.push_back(tenant_id);
                continue;
            }

            let Some(queue) = self.tenant_queues.get_mut(&tenant_id) else {
                continue;
            };
            let Some(waiter) = queue.pop_front() else {
                self.tenant_queues.remove(&tenant_id);
                continue;
            };

            if queue.is_empty() {
                self.tenant_queues.remove(&tenant_id);
            } else {
                self.tenant_order.push_back(tenant_id.clone());
            }

            self.queued_total = self.queued_total.saturating_sub(1);
            self.active_total += 1;
            *self.active_per_tenant.entry(tenant_id.clone()).or_default() += 1;
            self.admitted_total += 1;
            self.queue_wait_time_ms_total += waiter
                .enqueued_at
                .elapsed()
                .as_millis()
                .min(u128::from(u64::MAX)) as u64;

            if waiter.sender.send(()).is_ok() {
                return;
            }

            self.active_total = self.active_total.saturating_sub(1);
            if let Some(active) = self.active_per_tenant.get_mut(&tenant_id) {
                *active = active.saturating_sub(1);
                if *active == 0 {
                    self.active_per_tenant.remove(&tenant_id);
                }
            }
        }
    }

    fn active_for_tenant(&self, tenant_id: &str) -> usize {
        self.active_per_tenant.get(tenant_id).copied().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::{ExecutionScheduler, ExecutionSchedulerConfig};
    use tokio::time::{sleep, timeout, Duration};

    #[tokio::test]
    async fn scheduler_times_out_queued_requests() {
        let scheduler = ExecutionScheduler::new(ExecutionSchedulerConfig::new(1, 4, 50, 1));
        let permit = scheduler.acquire("tenant-a").await.unwrap();

        let error = scheduler.acquire("tenant-b").await.unwrap_err();
        assert!(error.to_string().contains("waited longer"));

        drop(permit);
    }

    #[tokio::test]
    async fn scheduler_rejects_when_queue_is_full() {
        let scheduler = ExecutionScheduler::new(ExecutionSchedulerConfig::new(1, 1, 1_000, 1));
        let permit = scheduler.acquire("tenant-a").await.unwrap();

        let queued = tokio::spawn({
            let scheduler = scheduler.clone();
            async move { scheduler.acquire("tenant-b").await.unwrap() }
        });
        sleep(Duration::from_millis(20)).await;

        let error = scheduler.acquire("tenant-c").await.unwrap_err();
        assert!(error.to_string().contains("queue is full"));

        drop(permit);
        drop(queued.await.unwrap());
    }

    #[tokio::test]
    async fn scheduler_rotates_between_tenants_fairly() {
        let scheduler = ExecutionScheduler::new(ExecutionSchedulerConfig::new(1, 4, 1_000, 1));
        let permit_a1 = scheduler.acquire("tenant-a").await.unwrap();

        let queued_a2 = tokio::spawn({
            let scheduler = scheduler.clone();
            async move { scheduler.acquire("tenant-a").await.unwrap() }
        });
        sleep(Duration::from_millis(20)).await;

        let queued_b1 = tokio::spawn({
            let scheduler = scheduler.clone();
            async move { scheduler.acquire("tenant-b").await.unwrap() }
        });
        sleep(Duration::from_millis(20)).await;

        drop(permit_a1);

        let permit_b1 = timeout(Duration::from_millis(200), queued_b1)
            .await
            .expect("tenant-b should acquire next")
            .unwrap();
        sleep(Duration::from_millis(50)).await;
        assert!(
            !queued_a2.is_finished(),
            "tenant-a should still be queued while tenant-b is active"
        );

        drop(permit_b1);

        let permit_a2 = timeout(Duration::from_millis(200), queued_a2)
            .await
            .expect("tenant-a should acquire after tenant-b completes")
            .unwrap();
        assert!(permit_a2.was_queued());
    }

    #[tokio::test]
    async fn scheduler_allows_other_tenant_when_only_blocked_waiters_are_queued() {
        let scheduler = ExecutionScheduler::new(ExecutionSchedulerConfig::new(2, 4, 1_000, 1));
        let permit_a1 = scheduler.acquire("tenant-a").await.unwrap();

        let queued_a2 = tokio::spawn({
            let scheduler = scheduler.clone();
            async move { scheduler.acquire("tenant-a").await.unwrap() }
        });
        sleep(Duration::from_millis(20)).await;

        let permit_b1 = timeout(Duration::from_millis(200), scheduler.acquire("tenant-b"))
            .await
            .expect("tenant-b should use spare global capacity")
            .unwrap();

        assert!(!permit_b1.was_queued());
        assert!(!queued_a2.is_finished());

        drop(permit_b1);
        drop(permit_a1);
        drop(queued_a2.await.unwrap());
    }
}
