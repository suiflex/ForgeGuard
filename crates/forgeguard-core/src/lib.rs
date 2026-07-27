pub mod config;
pub mod detector;
pub mod doctor;
pub mod duplication;
pub mod gate;
pub mod git;
pub mod hook;
pub mod init;
pub mod model;
pub mod report;
pub mod runner;
pub mod scanner;
pub mod update;

pub use config::{CommandConfig, ForgeGuardConfig, GuardMode};
pub use detector::{detect_project, ProjectDetection};
pub use doctor::{run_doctor, DoctorReport};
pub use gate::{run_gate, GateOptions};
pub use hook::{
    evaluate_stop_hook, ignore_forgeguard_artifacts, render_hook_decision, HookAgent, HookDecision,
};
pub use init::{
    initialize_global, initialize_project, AgentTarget, GlobalInitReport, InitOptions, InitReport,
};
pub use model::{CheckResult, Finding, GateReport, GateStatus, Severity};
pub use scanner::{scan_project, ScanOptions};
