const crypto = require("crypto");
const fs = require("fs");
const https = require("https");
const os = require("os");
const path = require("path");

const packageJson = require("../package.json");
const repository = "07calc/fyrer";
const releaseVersion = process.env.FYRER_VERSION || `v${packageJson.version}`;
const baseUrl = process.env.FYRER_BASE_URL ||
  `https://github.com/${repository}/releases/download/${releaseVersion}`;

function getTarget() {
  const arch = {
    x64: "x86_64",
    arm64: "aarch64",
  }[process.arch];

  if (!arch) {
    throw new Error(`Unsupported architecture: ${process.arch}`);
  }

  switch (process.platform) {
    case "linux":
      return `${arch}-unknown-linux-musl`;
    case "darwin":
      return `${arch}-apple-darwin`;
    case "win32":
      return `${arch}-pc-windows-msvc`;
    default:
      throw new Error(`Unsupported operating system: ${process.platform}`);
  }
}

function download(url, redirects = 0) {
  if (redirects > 5) {
    return Promise.reject(new Error("Too many redirects while downloading Fyrer"));
  }

  return new Promise((resolve, reject) => {
    https.get(url, (response) => {
      if (response.statusCode >= 300 && response.statusCode < 400 && response.headers.location) {
        response.resume();
        download(new URL(response.headers.location, url).toString(), redirects + 1)
          .then(resolve, reject);
        return;
      }

      if (response.statusCode !== 200) {
        response.resume();
        reject(new Error(`Download failed with HTTP ${response.statusCode}: ${url}`));
        return;
      }

      const chunks = [];
      response.on("data", (chunk) => chunks.push(chunk));
      response.on("end", () => resolve(Buffer.concat(chunks)));
      response.on("error", reject);
    }).on("error", reject);
  });
}

async function install() {
  const target = getTarget();
  const extension = process.platform === "win32" ? ".exe" : "";
  const asset = `fyrer-${target}${extension}`;
  const checksumAsset = `${asset}.sha256`;
  const binaryUrl = `${baseUrl}/${asset}`;
  const checksumUrl = `${baseUrl}/${checksumAsset}`;
  const binary = await download(binaryUrl);
  const checksum = (await download(checksumUrl)).toString("utf8").trim();
  const expected = checksum.split(/\s+/)[0].toLowerCase();
  const actual = crypto.createHash("sha256").update(binary).digest("hex");

  if (!expected || expected !== actual) {
    throw new Error(`Checksum verification failed for ${asset}`);
  }

  const vendorDirectory = path.join(__dirname, "..", "vendor");
  const binaryPath = path.join(vendorDirectory, `fyrer${extension}`);
  fs.mkdirSync(vendorDirectory, { recursive: true });
  fs.writeFileSync(binaryPath, binary, { mode: 0o755 });

  if (process.platform !== "win32") {
    fs.chmodSync(binaryPath, 0o755);
  }

  console.log(`Installed Fyrer ${releaseVersion} for ${os.platform()} ${process.arch}`);
}

install().catch((error) => {
  console.error(`Failed to install Fyrer: ${error.message}`);
  process.exit(1);
});
