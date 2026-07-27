use std::fs;

use forgeguard_core::update::{cached_notice, current_version};
use tempfile::tempdir;

fn write_cache(home: &std::path::Path, latest: &str) {
    let dir = home.join(".forgeguard");
    fs::create_dir_all(&dir).expect("create cache dir");
    fs::write(
        dir.join("update-check.json"),
        format!("{{\"checked_at\":0,\"latest\":\"{latest}\"}}"),
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
