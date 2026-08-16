#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const { spawn } = require("child_process");

const binaryName = process.platform === "win32" ? "fyrer.exe" : "fyrer";
const binaryPath = path.join(__dirname, "..", "vendor", binaryName);

if (!fs.existsSync(binaryPath)) {
  console.error("Fyrer binary is missing. Reinstall the package to run its postinstall script.");
  process.exit(1);
}

const child = spawn(binaryPath, process.argv.slice(2), {
  stdio: "inherit",
  windowsHide: false,
});

child.on("error", (error) => {
  console.error(`Failed to start Fyrer: ${error.message}`);
  process.exit(1);
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
  } else {
    process.exit(code === null ? 1 : code);
  }
});
