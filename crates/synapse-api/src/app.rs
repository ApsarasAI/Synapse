use synapse_core::{
    AuditLog, ExecutionScheduler, ExecutionSchedulerConfig, RuntimeRegistry, SandboxPool,
    SynapseConfig, SystemProviders, TenantQuotaConfig, TenantQuotaManager,
};

use crate::metrics::ExecutionMetrics;

#[derive(Clone, Debug)]
pub struct AppState {
    pool: SandboxPool,
    audit_log: AuditLog,
    tenant_quotas: TenantQuotaManager,
    scheduler: ExecutionScheduler,
    execution_metrics: ExecutionMetrics,
    runtime_registry: RuntimeRegistry,
}

impl AppState {
    pub fn new(pool: SandboxPool, audit_log: AuditLog, tenant_quotas: TenantQuotaManager) -> Self {
        Self::new_with_runtime_registry(pool, audit_log, tenant_quotas, RuntimeRegistry::default())
    }

    pub fn new_with_runtime_registry(
        pool: SandboxPool,
        audit_log: AuditLog,
        tenant_quotas: TenantQuotaManager,
        runtime_registry: RuntimeRegistry,
    ) -> Self {
        let _ = runtime_registry.bootstrap_system_defaults();
        let scheduler = ExecutionScheduler::new(ExecutionSchedulerConfig::new(
            pool.metrics().configured_size,
            tenant_quotas.config().max_queue_depth,
            tenant_quotas.config().max_queue_timeout_ms,
            tenant_quotas.config().max_concurrent_executions_per_tenant,
        ));
        Self {
            pool,
            audit_log,
            tenant_quotas,
            scheduler,
            execution_metrics: ExecutionMetrics::default(),
            runtime_registry,
        }
    }

    pub fn pool(&self) -> &SandboxPool {
        &self.pool
    }

    pub fn audit_log(&self) -> &AuditLog {
        &self.audit_log
    }

    pub fn tenant_quotas(&self) -> &TenantQuotaManager {
        &self.tenant_quotas
    }

    pub fn scheduler(&self) -> &ExecutionScheduler {
        &self.scheduler
    }

    pub fn execution_metrics(&self) -> &ExecutionMetrics {
        &self.execution_metrics
    }

    pub fn runtime_registry(&self) -> &RuntimeRegistry {
        &self.runtime_registry
    }
}

pub fn default_state() -> AppState {
    let config = SynapseConfig::from_providers(&SystemProviders);
    let runtime_registry = RuntimeRegistry::default();
    let _ = runtime_registry.bootstrap_system_defaults();
    AppState::new_with_runtime_registry(
        SandboxPool::new_with_runtime_registry(config.pool_size, runtime_registry.clone()),
        AuditLog::from_providers(&SystemProviders),
        TenantQuotaManager::new(TenantQuotaConfig {
            max_concurrent_executions_per_tenant: config.tenant_max_concurrency,
            max_requests_per_minute: config.tenant_max_requests_per_minute,
            max_timeout_ms: config.tenant_max_timeout_ms,
            max_cpu_time_limit_ms: config.tenant_max_cpu_time_limit_ms,
            max_memory_limit_mb: config.tenant_max_memory_limit_mb,
            max_queue_depth: config.max_queue_depth,
            max_queue_timeout_ms: config.max_queue_timeout_ms,
        }),
        runtime_registry,
    )
}
