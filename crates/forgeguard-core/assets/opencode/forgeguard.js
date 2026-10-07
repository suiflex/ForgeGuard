import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { join } from "node:path";
const GLOBAL_INSTALL = __FORGEGUARD_GLOBAL__;
let HOOK_COMMAND = "forgeguard hook stop --agent opencode";
if (GLOBAL_INSTALL) HOOK_COMMAND += " --global";
const [FORGEGUARD_BIN, ...HOOK_ARGS] = HOOK_COMMAND.split(" ");

function runForgeGuard(root, sessionId) {
  const payload = JSON.stringify({ cwd: root, sessionId });
  return new Promise((resolve, reject) => {
    const child = spawn(
      FORGEGUARD_BIN,
      ["--root", root, ...HOOK_ARGS],
      { cwd: root, stdio: ["pipe", "pipe", "pipe"] },
    );
    const output = { stdout: "", stderr: "" };
    for (const stream of ["stdout", "stderr"]) {
      child[stream].on("data", (chunk) => {
        if (output[stream].length < 65536) output[stream] += chunk;
      });
    }
    child.once("error", reject);
    child.once("close", (code) => {
      if (code !== 0) {
        reject(new Error(output.stderr.trim() || `forgeguard exited with code ${code}`));
        return;
      }
      try {
        resolve(output.stdout.trim() ? JSON.parse(output.stdout) : undefined);
      } catch (error) {
        reject(new Error(`ForgeGuard returned invalid hook output: ${error.message}`));
      }
    });
    child.stdin.end(payload);
  });
}

export const ForgeGuardPlugin = async ({ client, directory }) => ({
  event: async ({ event }) => {
    if (event.type !== "session.idle") return;

    const root = directory || process.cwd();
    if (
      GLOBAL_INSTALL &&
      existsSync(join(root, ".opencode/plugins/forgeguard.js"))
    ) return;

    const sessionId = event.properties?.sessionID;
    if (typeof sessionId !== "string" || !sessionId) return;

    try {
      const decision = await runForgeGuard(root, sessionId);
      if (decision?.action !== "revise" || typeof decision.reason !== "string") return;
      await client.session.prompt({
        path: { id: sessionId },
        body: { parts: [{ type: "text", text: decision.reason }] },
      });
    } catch (error) {
      console.error(`ForgeGuard OpenCode completion check failed: ${error.message}`);
    }
  },
});
