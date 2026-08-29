use std::fs;

use forgeguard_core::update::{
    cached_notice, cached_notice_for, check_for_update_for, current_version, UpdateCheck,
};
use tempfile::tempdir;

fn write_cache(home: &std::path::Path, latest: &str) {
    write_cache_with_time(home, latest, 0);
}

fn write_fresh_cache(home: &std::path::Path, latest: &str) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    write_cache_with_time(home, latest, now);
}

fn write_cache_with_time(home: &std::path::Path, latest: &str, checked_at: u64) {
    let dir = home.join(".forgeguard");
    fs::create_dir_all(&dir).expect("create cache dir");
    fs::write(
        dir.join("update-check.json"),
        format!("{{\"checked_at\":{checked_at},\"latest\":\"{latest}\"}}"),
    )
    .expect("write cache");
}

#[test]
fn cached_notice_appears_for_a_newer_release() {
    let home = tempdir().expect("temp home");
    write_cache(home.path(), "999.0.0");

    let notice = cached_notice(home.path());
    assert!(notice.is_some());
    assert!(notice.unwrap().contains("999.0.0"));
}

#[test]
fn cached_notice_is_absent_for_the_current_release() {
    let home = tempdir().expect("temp home");
    write_cache(home.path(), current_version());

    assert!(cached_notice(home.path()).is_none());
}

#[test]
fn cached_notice_is_absent_without_a_cache() {
    let home = tempdir().expect("temp home");
    assert!(cached_notice(home.path()).is_none());
}

#[test]
fn cached_notice_uses_the_supplied_binary_version() {
    let home = tempdir().expect("temp home");
    write_cache(home.path(), "0.11.2");

    assert!(cached_notice_for(home.path(), "0.11.2").is_none());
}

#[test]
fn check_for_update_returns_structured_info_when_newer() {
    let home = tempdir().expect("temp home");
    write_fresh_cache(home.path(), "1.0.0");

    let check = check_for_update_for(home.path(), false, "0.14.0").expect("check result");
    assert_eq!(
        check,
        UpdateCheck {
            current: "0.14.0".to_owned(),
            latest: "1.0.0".to_owned(),
            update_available: true,
        }
    );
}

#[test]
fn check_for_update_returns_structured_info_when_up_to_date() {
    let home = tempdir().expect("temp home");
    write_fresh_cache(home.path(), "0.14.0");

    let check = check_for_update_for(home.path(), false, "0.14.0").expect("check result");
    assert_eq!(
        check,
        UpdateCheck {
            current: "0.14.0".to_owned(),
            latest: "0.14.0".to_owned(),
            update_available: false,
        }
    );
}

#[test]
fn check_for_update_returns_structured_info_when_ahead_of_latest() {
    let home = tempdir().expect("temp home");
    write_fresh_cache(home.path(), "0.13.0");

    let check = check_for_update_for(home.path(), false, "0.14.0").expect("check result");
    assert_eq!(
        check,
        UpdateCheck {
            current: "0.14.0".to_owned(),
            latest: "0.13.0".to_owned(),
            update_available: false,
        }
    );
}
