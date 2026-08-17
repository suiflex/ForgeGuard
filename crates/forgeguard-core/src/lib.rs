pub mod baseline;
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
pub mod rules;
pub mod runner;
pub mod scanner;
pub mod update;

pub use baseline::{
    create_baseline, create_baseline_with_config, Baseline, BaselineEntry, BASELINE_FILE,
};
pub use config::{CommandConfig, FocusConfig, ForgeGuardConfig, GuardMode};
pub use detector::{detect_project, ProjectDetection};
pub use doctor::{run_doctor, DoctorReport};
pub use gate::{run_gate, GateOptions};
pub use hook::{
    evaluate_context_hook, evaluate_scope_hook, evaluate_stop_hook, ignore_forgeguard_artifacts,
    mark_task_ready, mark_task_ready_with_confidence, render_context_hook, render_hook_decision,
    render_scope_warning, start_task, start_task_with_contract, task_state, update_task_todos,
    GoalContract, HookAgent, HookDecision, TaskState, TaskStatus, TaskTodo,
};
pub use init::{
    detect_installed_agents, initialize_global, initialize_project, AgentTarget, GlobalInitReport,
    InitOptions, InitReport,
};
pub use model::{CheckResult, EvidenceConfidence, Finding, GateReport, GateStatus, Severity};
pub use rules::{LanguageCapability, RuleMetadata, LANGUAGE_CAPABILITIES, RULES};
pub use scanner::{scan_project, ScanOptions};
