import { spawn } from "node:child_process";

function runForgeGuard(command, context) {
  const root = context.workspaceDir || process.cwd();
  const payload = JSON.stringify({
    cwd: root,
    sessionId: context.sessionId || context.sessionKey || "default",
  });
  return new Promise((resolve) => {
    const failure = (message) =>
      command === "context"
        ? { prependContext: message }
        : { action: "revise", reason: message };
    const child = spawn(
      "forgeguard",
      ["--root", root, "hook", command, "--agent", "openclaw"],
      { cwd: root, stdio: ["pipe", "pipe", "pipe"] },
    );
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => {
      if (stdout.length < 65536) stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      if (stderr.length < 65536) stderr += chunk;
    });
    child.on("error", (error) =>
      resolve(failure(`ForgeGuard hook failed: ${error.message}`)),
    );
    child.on("close", (code) => {
      if (code !== 0) {
        resolve(failure(`ForgeGuard hook failed: ${stderr.trim() || `exit ${code}`}`));
        return;
      }
      try {
        resolve(stdout.trim() ? JSON.parse(stdout) : undefined);
      } catch (error) {
        resolve(failure(`ForgeGuard returned invalid hook output: ${error.message}`));
      }
    });
    child.stdin.end(payload);
  });
}

export default function register(api) {
  api.on(
    "before_prompt_build",
    (_event, context) => runForgeGuard("context", context),
    { timeoutMs: 5000 },
  );
  api.on(
    "before_agent_finalize",
    (_event, context) => runForgeGuard("stop", context),
    { timeoutMs: 600000 },
  );
}
