# Changelog

## [0.14.0](https://github.com/suiflex/ForgeGuard/compare/v0.13.0...v0.14.0) (2026-08-22)


### Features

* add new-code quality and security analysis ([122b4b2](https://github.com/suiflex/ForgeGuard/commit/122b4b211968a9d5c449d21dca1a54115f16d9e2))
* expand general guard across roles and agents ([be27f1c](https://github.com/suiflex/ForgeGuard/commit/be27f1c9fcea6c9e4d1e675bbf3296cf226364b5))
* productionize quality gates and expand General Guard ([2066544](https://github.com/suiflex/ForgeGuard/commit/2066544ee779d165ed684427ea40d1de45af39a4))
* productionize security and supply-chain gates ([431e773](https://github.com/suiflex/ForgeGuard/commit/431e773d8fdcdbfea7432c1a4b0b2a0a9873810b))


### Bug Fixes

* prevent duplicate global lifecycle hooks ([d8cbe62](https://github.com/suiflex/ForgeGuard/commit/d8cbe62f7d935356678aae66aef17cd188be820e))
* scope code guard modes to repositories ([0e1a485](https://github.com/suiflex/ForgeGuard/commit/0e1a48578c0304b893fd38559c6722898ddbe9ad))

## [0.13.1](https://github.com/suiflex/ForgeGuard/compare/v0.13.0...v0.13.1) (2026-08-22)

### Features

* expand General Guard with open-ended role profiles, acceptance coverage, evidence provenance, artifacts, and MCP/resource scope controls
* add global Hermes and OpenClaw integrations
* add changed-code security, coverage, duplication, SARIF, and opt-in supply-chain quality gates

### Bug Fixes

* prevent global General Guard hooks from duplicating project Code Guard lifecycle events
* preserve disabled global focus settings, repository-scoped Code Guard modes, unrelated hooks, and existing configuration during upgrades

### Documentation

* make the README problem-first and add concrete contributor onboarding, roadmap lanes, and GitHub contribution forms

## [0.13.0](https://github.com/suiflex/ForgeGuard/compare/v0.12.0...v0.13.0) (2026-08-17)


### Features

* **cli:** report policy files left as-is ([672bc4b](https://github.com/suiflex/ForgeGuard/commit/672bc4ba2bb934f3da6a8234caace6c8fad8cfc0))

## [0.12.0](https://github.com/suiflex/ForgeGuard/compare/v0.11.2...v0.12.0) (2026-08-17)


### Features

* **cli:** brand init with the shared terminal theme ([cfdf76f](https://github.com/suiflex/ForgeGuard/commit/cfdf76f000519142368e84da9f652fb47344ad58))
* **cli:** install init only for the agents a directory uses ([081ee20](https://github.com/suiflex/ForgeGuard/commit/081ee204451be8cdf789eb43d1b80d788074859a))
* **init:** install only for detected agents, ask before overwriting ([2c7f00d](https://github.com/suiflex/ForgeGuard/commit/2c7f00d1dfaf0a2ca1e8cb6665f6b476b4fad674))
* **init:** offer to refresh drifted files instead of deciding for you ([cf4ebb2](https://github.com/suiflex/ForgeGuard/commit/cf4ebb2e8d306c87478b0842186ccc78356d19a4))


### Bug Fixes

* **cli:** draw the real ForgeGuard mark in the banner ([a2a304e](https://github.com/suiflex/ForgeGuard/commit/a2a304e4a347481894ce4ebe133d4f82b2a3b713))

## [0.11.2](https://github.com/suiflex/ForgeGuard/compare/v0.11.1...v0.11.2) (2026-08-13)


### Miscellaneous Chores

* force a 0.11.2 release to validate Cargo.lock automation ([8ab564b](https://github.com/suiflex/ForgeGuard/commit/8ab564b77b76dfd2c050d45717c460457debf2f4))

## [0.11.1](https://github.com/suiflex/ForgeGuard/compare/v0.11.0...v0.11.1) (2026-08-13)


### Miscellaneous Chores

* force a 0.11.1 release to validate the fixed pipeline ([3c490a2](https://github.com/suiflex/ForgeGuard/commit/3c490a204c04e3dcc8b9be5c15d4cf8947eca413))

## [0.11.0](https://github.com/suiflex/ForgeGuard/compare/v0.10.0...v0.11.0) (2026-08-13)


### Features

* add update policy gate (auto/ask/off) ([a05b7ae](https://github.com/suiflex/ForgeGuard/commit/a05b7aeec6d330014acac9f2fe324c50795fc4b5))
* add update policy gate (auto/ask/off) ([dd2ef00](https://github.com/suiflex/ForgeGuard/commit/dd2ef00da4a2751aac08dbf63e310c37d3295306))


### Bug Fixes

* pin literal version in released crate's Cargo.toml ([17fbe6d](https://github.com/suiflex/ForgeGuard/commit/17fbe6d31e148ec9f0d6136919e35df17b1dd49e))
* pin literal version in released crate's Cargo.toml ([4479eab](https://github.com/suiflex/ForgeGuard/commit/4479eab3fc3a397daf93f215b18b7901ce4223f1))
