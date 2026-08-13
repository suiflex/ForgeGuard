#!/usr/bin/env node

const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

const binaryName = process.platform === "win32" ? "forgeguard.exe" : "forgeguard";
const binaryPath = path.join(__dirname, "..", "vendor", binaryName);

if (!fs.existsSync(binaryPath)) {
  console.error("ForgeGuard binary is missing. Reinstall @suiflex/forgeguard.");
  process.exit(1);
}

const result = spawnSync(binaryPath, process.argv.slice(2), {
  stdio: "inherit",
  windowsHide: false,
});

if (result.error) {
  console.error(`Unable to start ForgeGuard: ${result.error.message}`);
  process.exit(1);
}

process.exit(result.status ?? 1);
