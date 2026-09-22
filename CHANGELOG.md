# Changelog

All notable changes to ForgeGuard will be documented here by Release Please.

## ForgeGuard: [0.18.0](https://github.com/suiflex/ForgeGuard/compare/v0.17.0...v0.18.0) (2026-09-20)


### Features

* **memory:** fix AST extraction, cypher parsing, and add OMP harness support ([588361b](https://github.com/suiflex/ForgeGuard/commit/588361b1bb028fccd08340c5e996f245a52c92be))


### Bug Fixes

* **memory:** improve extraction, indexing, and scanner reliability ([0e70391](https://github.com/suiflex/ForgeGuard/commit/0e7039163a8e5ac738914a3e9ff092b85440a46c))
* satisfy Guardener complexity gate ([18b13ed](https://github.com/suiflex/ForgeGuard/commit/18b13ed88662c7e3cb3664475923aee0325a96ed))

## ForgeGuard: [0.17.0](https://github.com/suiflex/ForgeGuard/compare/v0.16.0...v0.17.0) (2026-09-17)


### Features

* **init:** build the code graph and register MCP per repository ([f72a374](https://github.com/suiflex/ForgeGuard/commit/f72a3749f5507eff5799b1f8a8250561012f0a88))
* **init:** build the code graph and register MCP per repository ([5c7ed34](https://github.com/suiflex/ForgeGuard/commit/5c7ed34916f9732dc3c81bea0eb306a618855006))


### Bug Fixes

* **init:** register MCP for every expanded agent and dedupe ignore entries ([52a310d](https://github.com/suiflex/ForgeGuard/commit/52a310d289f9078bcc51be69ad12b946aec08ca5))

## ForgeGuard: [0.16.0](https://github.com/suiflex/ForgeGuard/compare/v0.15.0...v0.16.0) (2026-09-17)


### Features

* **cli:** add MCP server and move harness paths to kurir ([d42d2af](https://github.com/suiflex/ForgeGuard/commit/d42d2af3f3fe90af0588227f37da906e8e8aeb57))
* **cli:** serve and register ForgeGuard over MCP ([dc564b4](https://github.com/suiflex/ForgeGuard/commit/dc564b47d70b5fd621ae67d1ef96899886daebaa))
* **memory:** add a persistent code-intelligence layer ([195d6ea](https://github.com/suiflex/ForgeGuard/commit/195d6ea84a39edfba4f3c0b749e2405385fcf05b))
* **memory:** persistent code-intelligence layer ([ebd7c40](https://github.com/suiflex/ForgeGuard/commit/ebd7c4056ae7e9c3ec537e964514bcd2f2962e5c))


### Bug Fixes

* **ci:** consolidate changelog to root and fix fold script ([9621e26](https://github.com/suiflex/ForgeGuard/commit/9621e2690209744b4caab86299479f5c4d7db8c7))

## [0.19.0](https://github.com/suiflex/ForgeGuard/compare/v0.18.0...v0.19.0) (2026-09-22)


### Features

* **detector:** add C/C++ build and test command presets ([20f6f01](https://github.com/suiflex/ForgeGuard/commit/20f6f013b7a06bfe9e48449c5b7cd30f86f12fb7))
* **detector:** add C/C++ build and test command presets ([5ffa68d](https://github.com/suiflex/ForgeGuard/commit/5ffa68d14775f7bc61c90189faf591ba41450923))


### Bug Fixes

* **ci:** scope release-please to root workspace ([3205186](https://github.com/suiflex/ForgeGuard/commit/32051865456928f411b67e0a23a53bad72edfa46))
* **ci:** scope release-please to root workspace ([9e37ed9](https://github.com/suiflex/ForgeGuard/commit/9e37ed9af8efbd970a31f6a8a79e1837056398f8))
* **doctor:** resolve ./-prefixed wrapper tools against the repo root ([5f40b8c](https://github.com/suiflex/ForgeGuard/commit/5f40b8c9ec55876629521f9a60a5b90d6cd2d4bb))
* **doctor:** resolve `./`-prefixed wrapper tools against the repo root ([01dc23f](https://github.com/suiflex/ForgeGuard/commit/01dc23f094c018b86f7ea0dc5125bef43bf6be29))
* **update:** ignore prerelease/build tags and accept v-prefixed versions ([ad467d5](https://github.com/suiflex/ForgeGuard/commit/ad467d5c59bfe87f470c39dfb1580947c60130aa))
* **update:** ignore prerelease/build tags and accept v-prefixed versions ([333d230](https://github.com/suiflex/ForgeGuard/commit/333d2300b66ffe2f51d7f69ea94db0ca67686b54))

## [0.15.0](https://github.com/suiflex/ForgeGuard/compare/v0.14.0...v0.15.0) (2026-08-29)


### Features

* **cli:** execute in-place installation on forgeguard update ([e441acd](https://github.com/suiflex/ForgeGuard/commit/e441acdd67bbb0441c0a7444420cb147d8033fec))
* install updates directly from forgeguard update ([a246210](https://github.com/suiflex/ForgeGuard/commit/a246210be9bfd29cbbec8128e9a2eae1e32d4bd0))

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
