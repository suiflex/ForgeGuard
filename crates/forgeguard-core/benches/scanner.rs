use std::{
    fs,
    time::{Duration, Instant},
};

use forgeguard_core::{config::ScanConfig, scan_project, ScanOptions};

fn main() {
    for lines in [100, 1_000, 10_000] {
        let elapsed = measure(lines);
        println!("{lines:>6} TypeScript lines: {:>8.2?}", elapsed);
    }
    for queries in [100, 1_000, 10_000] {
        let elapsed = measure_sql(queries);
        println!("{queries:>6} SQL queries:      {:>8.2?}", elapsed);
    }
}

fn measure(lines: usize) -> Duration {
    let directory = std::env::temp_dir().join(format!("forgeguard-bench-{}", std::process::id()));
    fs::create_dir(&directory).expect("create benchmark directory");
    fs::write(
        directory.join("service.ts"),
        "values.map((value) => value + 1);\n".repeat(lines),
    )
    .expect("write benchmark source");

    let started = Instant::now();
    for _ in 0..5 {
        scan_project(&directory, &ScanConfig::default(), &ScanOptions::default())
            .expect("scan benchmark source");
    }
    let elapsed = started.elapsed() / 5;
    fs::remove_dir_all(directory).expect("remove benchmark directory");
    elapsed
}

fn measure_sql(queries: usize) -> Duration {
    let directory = std::env::temp_dir().join(format!("forgeguard-bench-{}", std::process::id()));
    fs::create_dir(&directory).expect("create benchmark directory");
    fs::write(
        directory.join("queries.sql"),
        "SELECT * FROM users;\n".repeat(queries),
    )
    .expect("write benchmark source");

    let started = Instant::now();
    for _ in 0..5 {
        scan_project(&directory, &ScanConfig::default(), &ScanOptions::default())
            .expect("scan benchmark source");
    }
    let elapsed = started.elapsed() / 5;
    fs::remove_dir_all(directory).expect("remove benchmark directory");
    elapsed
}
