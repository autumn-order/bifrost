//! Tests for WorkerPoolConfig
//!
//! These tests verify the configuration struct's behavior including:
//! - Default values
//! - Custom configuration creation
//! - Duration conversions
//! - Configuration cloning
//! - Testing-specific configuration

use std::time::Duration;

use crate::server::worker::pool::WorkerPoolConfig;

#[test]
fn test_default_config() {
    let config = WorkerPoolConfig::default();

    assert_eq!(
        config.max_concurrent_jobs, 50,
        "Default max_concurrent_jobs should be 50"
    );
    assert_eq!(
        config.dispatcher_count, 2,
        "Default dispatcher_count should be 2"
    );
    assert_eq!(
        config.poll_interval_ms, 50,
        "Default poll_interval_ms should be 50"
    );
    assert_eq!(
        config.job_timeout_seconds, 60,
        "Default job_timeout_seconds should be 60 (1 minute)"
    );
    assert_eq!(
        config.shutdown_timeout_seconds, 5,
        "Default shutdown_timeout_seconds should be 5"
    );
    assert_eq!(
        config.cleanup_interval_ms,
        5 * 60 * 1000,
        "Default cleanup_interval_ms should be 300000 (5 minutes)"
    );
}

#[test]
fn test_new_config_with_custom_max_concurrent_jobs() {
    let config = WorkerPoolConfig::new(100);

    assert_eq!(
        config.max_concurrent_jobs, 100,
        "max_concurrent_jobs should be 100"
    );
    assert_eq!(
        config.dispatcher_count, 2,
        "dispatcher_count should be default 2"
    );
    assert_eq!(
        config.poll_interval_ms, 50,
        "poll_interval_ms should be default 50"
    );
    assert_eq!(
        config.job_timeout_seconds, 60,
        "job_timeout_seconds should be default 60"
    );
    assert_eq!(
        config.shutdown_timeout_seconds, 5,
        "shutdown_timeout_seconds should be default 5"
    );
    assert_eq!(
        config.cleanup_interval_ms,
        5 * 60 * 1000,
        "cleanup_interval_ms should be default 300000"
    );
}

#[test]
fn test_job_timeout_conversion() {
    let config = WorkerPoolConfig {
        max_concurrent_jobs: 50,
        dispatcher_count: 2,
        poll_interval_ms: 50,
        job_timeout_seconds: 120,
        shutdown_timeout_seconds: 5,
        cleanup_interval_ms: 5 * 60 * 1000,
    };

    let timeout = config.job_timeout();
    assert_eq!(
        timeout,
        Duration::from_secs(120),
        "job_timeout() should return Duration from seconds"
    );
}

#[test]
fn test_poll_interval_conversion() {
    let config = WorkerPoolConfig {
        max_concurrent_jobs: 50,
        dispatcher_count: 2,
        poll_interval_ms: 100,
        job_timeout_seconds: 300,
        shutdown_timeout_seconds: 5,
        cleanup_interval_ms: 5 * 60 * 1000,
    };

    let interval = config.poll_interval();
    assert_eq!(
        interval,
        Duration::from_millis(100),
        "poll_interval() should return Duration from milliseconds"
    );
}

#[test]
fn test_shutdown_timeout_conversion() {
    let config = WorkerPoolConfig {
        max_concurrent_jobs: 50,
        dispatcher_count: 2,
        poll_interval_ms: 50,
        job_timeout_seconds: 300,
        shutdown_timeout_seconds: 10,
        cleanup_interval_ms: 5 * 60 * 1000,
    };

    let timeout = config.shutdown_timeout();
    assert_eq!(
        timeout,
        Duration::from_secs(10),
        "shutdown_timeout() should return Duration from seconds"
    );
}

#[test]
fn test_cleanup_interval_conversion() {
    let config = WorkerPoolConfig {
        max_concurrent_jobs: 50,
        dispatcher_count: 2,
        poll_interval_ms: 50,
        job_timeout_seconds: 300,
        shutdown_timeout_seconds: 5,
        cleanup_interval_ms: 60000,
    };

    let interval = config.cleanup_interval();
    assert_eq!(
        interval,
        Duration::from_millis(60000),
        "cleanup_interval() should return Duration from milliseconds"
    );
}

#[test]
fn test_config_clone() {
    let config1 = WorkerPoolConfig::new(80);
    let config2 = config1.clone();

    assert_eq!(
        config1.max_concurrent_jobs, config2.max_concurrent_jobs,
        "Cloned config should have same max_concurrent_jobs"
    );
    assert_eq!(
        config1.dispatcher_count, config2.dispatcher_count,
        "Cloned config should have same dispatcher_count"
    );
}

#[test]
fn test_config_with_custom_timeouts() {
    let mut config = WorkerPoolConfig::new(50);
    config.job_timeout_seconds = 10;
    config.shutdown_timeout_seconds = 3;
    config.cleanup_interval_ms = 30000;

    assert_eq!(config.job_timeout(), Duration::from_secs(10));
    assert_eq!(config.shutdown_timeout(), Duration::from_secs(3));
    assert_eq!(config.cleanup_interval(), Duration::from_millis(30000));
}

#[test]
fn test_realistic_production_config() {
    let mut config = WorkerPoolConfig::new(80);
    config.dispatcher_count = 2;
    config.poll_interval_ms = 50;
    config.job_timeout_seconds = 60;
    config.shutdown_timeout_seconds = 5;
    config.cleanup_interval_ms = 5 * 60 * 1000;

    assert_eq!(config.max_concurrent_jobs, 80);
    assert_eq!(config.dispatcher_count, 2);
    assert_eq!(config.job_timeout(), Duration::from_secs(60));
    assert_eq!(config.shutdown_timeout(), Duration::from_secs(5));
    assert_eq!(
        config.cleanup_interval(),
        Duration::from_millis(5 * 60 * 1000)
    );
}

#[test]
fn test_short_timeouts_for_testing() {
    let mut config = WorkerPoolConfig::new(10);
    config.poll_interval_ms = 10;
    config.job_timeout_seconds = 1;
    config.shutdown_timeout_seconds = 1;
    config.cleanup_interval_ms = 100;

    assert_eq!(config.poll_interval(), Duration::from_millis(10));
    assert_eq!(config.job_timeout(), Duration::from_secs(1));
    assert_eq!(config.shutdown_timeout(), Duration::from_secs(1));
    assert_eq!(config.cleanup_interval(), Duration::from_millis(100));
}
