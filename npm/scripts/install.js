const crypto = require("node:crypto");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { execFileSync } = require("node:child_process");
const https = require("node:https");

const packageRoot = path.join(__dirname, "..");
const vendorDirectory = path.join(packageRoot, "vendor");
const version = require(path.join(packageRoot, "package.json")).version;
const platform = process.platform;
const architecture = process.arch;

const platforms = {
  "linux:x64": ["forgeguard-linux-x86_64.tar.gz", "tar"],
  "linux:arm64": ["forgeguard-linux-aarch64.tar.gz", "tar"],
  "darwin:x64": ["forgeguard-macos-x86_64.tar.gz", "tar"],
  "darwin:arm64": ["forgeguard-macos-aarch64.tar.gz", "tar"],
  "win32:x64": ["forgeguard-windows-x86_64.zip", "zip"],
  "win32:arm64": ["forgeguard-windows-aarch64.zip", "zip"],
};

const target = platforms[`${platform}:${architecture}`];
if (!target) {
  throw new Error(`Unsupported ForgeGuard platform: ${platform}/${architecture}`);
}

const [archiveName, archiveType] = target;
const baseUrl = `https://github.com/suiflex/ForgeGuard/releases/download/v${version}`;
const archivePath = path.join(os.tmpdir(), `${archiveName}-${process.pid}`);
const checksumPath = `${archivePath}.sha256`;

function download(url, destination) {
  return new Promise((resolve, reject) => {
    const request = https.get(url, (response) => {
      if (response.statusCode >= 300 && response.statusCode < 400 && response.headers.location) {
        response.resume();
        download(new URL(response.headers.location, url), destination).then(resolve, reject);
        return;
      }
      if (response.statusCode !== 200) {
        response.resume();
        reject(new Error(`Download failed (${response.statusCode}): ${url}`));
        return;
      }
      const output = fs.createWriteStream(destination);
      response.pipe(output);
      output.on("finish", () => output.close(resolve));
      output.on("error", reject);
    });
    request.on("error", reject);
  });
}

function checksum(file) {
  return new Promise((resolve, reject) => {
    const hash = crypto.createHash("sha256");
    const input = fs.createReadStream(file);
    input.on("error", reject);
    input.on("data", (chunk) => hash.update(chunk));
    input.on("end", () => resolve(hash.digest("hex")));
  });
}

async function main() {
  fs.mkdirSync(vendorDirectory, { recursive: true });
  await download(`${baseUrl}/${archiveName}`, archivePath);
  await download(`${baseUrl}/${archiveName}.sha256`, checksumPath);

  const expected = fs.readFileSync(checksumPath, "utf8").trim().split(/\s+/)[0].toLowerCase();
  const actual = await checksum(archivePath);
  if (!/^[a-f0-9]{64}$/.test(expected) || expected !== actual) {
    throw new Error(`Checksum verification failed for ${archiveName}`);
  }

  if (archiveType === "tar") {
    execFileSync("tar", ["-xzf", archivePath, "-C", vendorDirectory]);
  } else {
    execFileSync("powershell.exe", [
      "-NoProfile", "-NonInteractive", "-Command",
      `Expand-Archive -LiteralPath '${archivePath.replaceAll("'", "''")}' -DestinationPath '${vendorDirectory.replaceAll("'", "''")}' -Force`,
    ]);
  }

  const binary = path.join(vendorDirectory, platform === "win32" ? "forgeguard.exe" : "forgeguard");
  if (!fs.existsSync(binary)) {
    throw new Error(`Release archive did not contain ${path.basename(binary)}`);
  }
  if (platform !== "win32") fs.chmodSync(binary, 0o755);
  fs.rmSync(archivePath, { force: true });
  fs.rmSync(checksumPath, { force: true });
}

main().catch((error) => {
  console.error(`@suiflex/forgeguard install failed: ${error.message}`);
  process.exit(1);
});
