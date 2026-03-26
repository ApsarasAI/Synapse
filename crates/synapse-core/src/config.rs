use crate::providers::Providers;

pub const DEFAULT_POOL_SIZE: usize = 4;
pub const POOL_SIZE_ENV: &str = "SYNAPSE_POOL_SIZE";
pub const DEFAULT_MAX_TENANT_CONCURRENCY: usize = 2;
pub const DEFAULT_MAX_REQUESTS_PER_MINUTE: usize = 120;
pub const DEFAULT_MAX_TIMEOUT_MS: u64 = 30_000;
pub const DEFAULT_MAX_CPU_TIME_LIMIT_MS: u64 = 30_000;
pub const DEFAULT_MAX_MEMORY_LIMIT_MB: u32 = 512;
pub const DEFAULT_MAX_QUEUE_DEPTH: usize = 32;
pub const DEFAULT_MAX_QUEUE_TIMEOUT_MS: u64 = 5_000;
pub const TENANT_MAX_CONCURRENCY_ENV: &str = "SYNAPSE_TENANT_MAX_CONCURRENCY";
pub const TENANT_MAX_REQUESTS_PER_MINUTE_ENV: &str = "SYNAPSE_TENANT_MAX_REQUESTS_PER_MINUTE";
pub const TENANT_MAX_TIMEOUT_MS_ENV: &str = "SYNAPSE_TENANT_MAX_TIMEOUT_MS";
pub const TENANT_MAX_CPU_TIME_LIMIT_MS_ENV: &str = "SYNAPSE_TENANT_MAX_CPU_TIME_LIMIT_MS";
pub const TENANT_MAX_MEMORY_LIMIT_MB_ENV: &str = "SYNAPSE_TENANT_MAX_MEMORY_LIMIT_MB";
pub const MAX_QUEUE_DEPTH_ENV: &str = "SYNAPSE_MAX_QUEUE_DEPTH";
pub const MAX_QUEUE_TIMEOUT_MS_ENV: &str = "SYNAPSE_MAX_QUEUE_TIMEOUT_MS";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SynapseConfig {
    pub pool_size: usize,
    pub tenant_max_concurrency: usize,
    pub tenant_max_requests_per_minute: usize,
    pub tenant_max_timeout_ms: u64,
    pub tenant_max_cpu_time_limit_ms: u64,
    pub tenant_max_memory_limit_mb: u32,
    pub max_queue_depth: usize,
    pub max_queue_timeout_ms: u64,
}

impl Default for SynapseConfig {
    fn default() -> Self {
        Self {
            pool_size: DEFAULT_POOL_SIZE,
            tenant_max_concurrency: DEFAULT_MAX_TENANT_CONCURRENCY,
            tenant_max_requests_per_minute: DEFAULT_MAX_REQUESTS_PER_MINUTE,
            tenant_max_timeout_ms: DEFAULT_MAX_TIMEOUT_MS,
            tenant_max_cpu_time_limit_ms: DEFAULT_MAX_CPU_TIME_LIMIT_MS,
            tenant_max_memory_limit_mb: DEFAULT_MAX_MEMORY_LIMIT_MB,
            max_queue_depth: DEFAULT_MAX_QUEUE_DEPTH,
            max_queue_timeout_ms: DEFAULT_MAX_QUEUE_TIMEOUT_MS,
        }
    }
}

impl SynapseConfig {
    pub fn from_providers(providers: &dyn Providers) -> Self {
        let pool_size = read_env_or_default(providers, POOL_SIZE_ENV, DEFAULT_POOL_SIZE);
        let tenant_max_concurrency = read_env_or_default(
            providers,
            TENANT_MAX_CONCURRENCY_ENV,
            DEFAULT_MAX_TENANT_CONCURRENCY,
        );
        let tenant_max_requests_per_minute = read_env_or_default(
            providers,
            TENANT_MAX_REQUESTS_PER_MINUTE_ENV,
            DEFAULT_MAX_REQUESTS_PER_MINUTE,
        );
        let tenant_max_timeout_ms =
            read_env_or_default(providers, TENANT_MAX_TIMEOUT_MS_ENV, DEFAULT_MAX_TIMEOUT_MS);
        let tenant_max_cpu_time_limit_ms = read_env_or_default(
            providers,
            TENANT_MAX_CPU_TIME_LIMIT_MS_ENV,
            DEFAULT_MAX_CPU_TIME_LIMIT_MS,
        );
        let tenant_max_memory_limit_mb = read_env_or_default(
            providers,
            TENANT_MAX_MEMORY_LIMIT_MB_ENV,
            DEFAULT_MAX_MEMORY_LIMIT_MB,
        );
        let max_queue_depth =
            read_env_or_default(providers, MAX_QUEUE_DEPTH_ENV, DEFAULT_MAX_QUEUE_DEPTH);
        let max_queue_timeout_ms = read_env_or_default(
            providers,
            MAX_QUEUE_TIMEOUT_MS_ENV,
            DEFAULT_MAX_QUEUE_TIMEOUT_MS,
        );

        Self {
            pool_size,
            tenant_max_concurrency,
            tenant_max_requests_per_minute,
            tenant_max_timeout_ms,
            tenant_max_cpu_time_limit_ms,
            tenant_max_memory_limit_mb,
            max_queue_depth,
            max_queue_timeout_ms,
        }
    }
}

fn read_env_or_default<T>(providers: &dyn Providers, key: &str, default: T) -> T
where
    T: std::str::FromStr + PartialOrd + From<u8> + Copy,
{
    providers
        .env_var(key)
        .and_then(|value| value.parse().ok())
        .filter(|value: &T| *value > T::from(0))
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::{SynapseConfig, DEFAULT_POOL_SIZE, POOL_SIZE_ENV};
    use crate::providers::Providers;
    use std::{collections::HashMap, ffi::OsString, path::PathBuf};

    #[derive(Debug, Default)]
    struct FakeProviders {
        env: HashMap<String, String>,
    }

    impl Providers for FakeProviders {
        fn env_var(&self, key: &str) -> Option<String> {
            self.env.get(key).cloned()
        }

        fn env_var_os(&self, key: &str) -> Option<OsString> {
            self.env.get(key).map(|v| OsString::from(v.clone()))
        }

        fn temp_dir(&self) -> PathBuf {
            PathBuf::from("/tmp")
        }

        fn process_id(&self) -> u32 {
            1
        }

        fn now_unix_nanos(&self) -> u128 {
            1
        }
    }

    #[test]
    fn defaults_when_env_missing() {
        let fake = FakeProviders::default();
        let config = SynapseConfig::from_providers(&fake);
        assert_eq!(config.pool_size, DEFAULT_POOL_SIZE);
    }

    #[test]
    fn defaults_when_env_invalid() {
        let mut fake = FakeProviders::default();
        fake.env
            .insert(POOL_SIZE_ENV.to_string(), "nope".to_string());
        let config = SynapseConfig::from_providers(&fake);
        assert_eq!(config.pool_size, DEFAULT_POOL_SIZE);
    }

    #[test]
    fn defaults_when_env_zero() {
        let mut fake = FakeProviders::default();
        fake.env.insert(POOL_SIZE_ENV.to_string(), "0".to_string());
        let config = SynapseConfig::from_providers(&fake);
        assert_eq!(config.pool_size, DEFAULT_POOL_SIZE);
    }

    #[test]
    fn reads_pool_size_from_env() {
        let mut fake = FakeProviders::default();
        fake.env.insert(POOL_SIZE_ENV.to_string(), "8".to_string());
        let config = SynapseConfig::from_providers(&fake);
        assert_eq!(config.pool_size, 8);
    }
}
