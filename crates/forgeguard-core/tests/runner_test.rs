#[cfg(unix)]
#[test]
fn command_timeout_stops_long_running_check() {
    use std::time::{Duration, Instant};

    use forgeguard_core::{runner::run_checks, CommandConfig};
    use tempfile::tempdir;

    let directory = tempdir().expect("temp directory");
    let started = Instant::now();
    let results = run_checks(
        directory.path(),
        &[CommandConfig {
            name: "slow".to_owned(),
            command: "sleep 2".to_owned(),
            required: true,
            enabled: true,
            timeout_seconds: 0,
        }],
    );

    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(!results[0].success);
    assert!(results[0].output.contains("timed out"));
}
