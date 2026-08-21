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

#[cfg(unix)]
#[test]
fn supply_chain_checks_run_only_for_dependency_changes_and_reuse_the_cache() {
    use std::{fs, path::PathBuf};

    use forgeguard_core::{runner::run_checks_for_changes, CommandConfig};
    use tempfile::tempdir;

    let directory = tempdir().expect("temp directory");
    fs::write(directory.path().join("Cargo.lock"), "version = 3\n").expect("write lockfile");
    let command = CommandConfig {
        name: "dependency-audit".to_owned(),
        command: "printf run >> .counter".to_owned(),
        required: true,
        enabled: true,
        timeout_seconds: 10,
    };

    let unrelated = run_checks_for_changes(
        directory.path(),
        std::slice::from_ref(&command),
        Some(&[PathBuf::from("src/lib.rs")]),
    );
    assert!(unrelated.is_empty());

    let changed = [PathBuf::from("Cargo.lock")];
    let first = run_checks_for_changes(
        directory.path(),
        std::slice::from_ref(&command),
        Some(&changed),
    );
    let second = run_checks_for_changes(
        directory.path(),
        std::slice::from_ref(&command),
        Some(&changed),
    );
    assert!(first[0].success && !first[0].cached);
    assert!(second[0].success && second[0].cached);
    assert_eq!(
        fs::read_to_string(directory.path().join(".counter")).expect("read counter"),
        "run"
    );

    fs::write(directory.path().join("Cargo.lock"), "version = 4\n").expect("change lockfile");
    let third = run_checks_for_changes(directory.path(), &[command], Some(&changed));
    assert!(third[0].success && !third[0].cached);
    assert_eq!(
        fs::read_to_string(directory.path().join(".counter")).expect("read counter"),
        "runrun"
    );
}

#[cfg(unix)]
#[test]
fn sbom_output_is_persisted_and_missing_artifacts_invalidate_the_cache() {
    use std::{fs, path::PathBuf};

    use forgeguard_core::{runner::run_checks_for_changes, CommandConfig};
    use tempfile::tempdir;

    let directory = tempdir().expect("temp directory");
    fs::write(directory.path().join("package-lock.json"), "{}\n").expect("write lockfile");
    let command = CommandConfig {
        name: "sbom".to_owned(),
        command: "printf '{\"bomFormat\":\"CycloneDX\"}'".to_owned(),
        required: true,
        enabled: true,
        timeout_seconds: 10,
    };
    let changed = [PathBuf::from("package-lock.json")];

    let first = run_checks_for_changes(
        directory.path(),
        std::slice::from_ref(&command),
        Some(&changed),
    );
    let artifact = directory.path().join(".forgeguard/reports/sbom/sbom.json");
    assert!(first[0].success && !first[0].cached);
    assert_eq!(
        fs::read_to_string(&artifact).expect("read SBOM"),
        "{\"bomFormat\":\"CycloneDX\"}"
    );

    fs::remove_file(&artifact).expect("remove generated artifact");
    let regenerated = run_checks_for_changes(directory.path(), &[command], Some(&changed));
    assert!(regenerated[0].success && !regenerated[0].cached);
    assert!(artifact.is_file());
}

#[cfg(unix)]
#[test]
fn failed_supply_chain_checks_are_never_cached() {
    use std::{fs, path::PathBuf};

    use forgeguard_core::{runner::run_checks_for_changes, CommandConfig};
    use tempfile::tempdir;

    let directory = tempdir().expect("temp directory");
    fs::write(directory.path().join("Cargo.lock"), "version = 3\n").expect("write lockfile");
    let command = CommandConfig {
        name: "dependency-audit".to_owned(),
        command: "printf run >> .counter; exit 1".to_owned(),
        required: true,
        enabled: true,
        timeout_seconds: 10,
    };
    let changed = [PathBuf::from("Cargo.lock")];

    let first = run_checks_for_changes(
        directory.path(),
        std::slice::from_ref(&command),
        Some(&changed),
    );
    let second = run_checks_for_changes(directory.path(), &[command], Some(&changed));

    assert!(!first[0].success && !second[0].success);
    assert_eq!(
        fs::read_to_string(
            directory
                .path()
                .join(".forgeguard/reports/supply-chain/dependency-audit.log")
        )
        .expect("read full audit output"),
        ""
    );
    assert_eq!(
        fs::read_to_string(directory.path().join(".counter")).expect("read counter"),
        "runrun"
    );
}

#[cfg(unix)]
#[test]
fn sbom_checks_fail_closed_without_valid_json_and_collect_workspace_outputs() {
    use std::{fs, path::PathBuf};

    use forgeguard_core::{runner::run_checks_for_changes, CommandConfig};
    use tempfile::tempdir;

    let directory = tempdir().expect("temp directory");
    fs::write(directory.path().join("Cargo.lock"), "version = 3\n").expect("write lockfile");
    let changed = [PathBuf::from("Cargo.lock")];
    let invalid = CommandConfig {
        name: "sbom-invalid".to_owned(),
        command: "printf completed".to_owned(),
        required: true,
        enabled: true,
        timeout_seconds: 10,
    };
    let failed = run_checks_for_changes(directory.path(), &[invalid], Some(&changed));
    assert!(!failed[0].success);
    assert!(failed[0].output.contains("no valid JSON artifact"));

    let workspace = CommandConfig {
        name: "sbom-cargo".to_owned(),
        command: "mkdir -p member; printf '{}' > forgeguard-sbom.cdx.json; printf '{}' > member/forgeguard-sbom.cdx.json".to_owned(),
        required: true,
        enabled: true,
        timeout_seconds: 10,
    };
    let generated = run_checks_for_changes(directory.path(), &[workspace], Some(&changed));
    assert!(generated[0].success);
    assert!(directory
        .path()
        .join(".forgeguard/reports/sbom/sbom-cargo/bom.json")
        .is_file());
    assert!(directory
        .path()
        .join(".forgeguard/reports/sbom/sbom-cargo/member/bom.json")
        .is_file());
    assert!(!directory.path().join("forgeguard-sbom.cdx.json").exists());
}
