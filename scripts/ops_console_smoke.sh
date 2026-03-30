#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if ! command -v python3 >/dev/null 2>&1; then
  echo "python3 is required for ops console smoke" >&2
  exit 1
fi

if ! command -v curl >/dev/null 2>&1; then
  echo "curl is required for ops console smoke" >&2
  exit 1
fi

if ! command -v node >/dev/null 2>&1; then
  echo "node is required for ops console smoke" >&2
  exit 1
fi

if ! command -v npm >/dev/null 2>&1; then
  echo "npm is required for ops console smoke" >&2
  exit 1
fi

chrome_bin="${CHROME_BIN:-}"
if [[ -z "$chrome_bin" ]]; then
  for candidate in google-chrome google-chrome-stable chromium chromium-browser; do
    if command -v "$candidate" >/dev/null 2>&1; then
      chrome_bin="$candidate"
      break
    fi
  done
fi

if [[ -z "$chrome_bin" ]]; then
  echo "google-chrome/chromium is required for ops console smoke" >&2
  exit 1
fi

chrome_bin="$(command -v "$chrome_bin")"

smoke_root="$(mktemp -d)"
runtime_store="$smoke_root/runtime-store"
audit_root="$smoke_root/audit"
node_root="$smoke_root/node-check"
download_root="$smoke_root/downloads"
listen="${SYNAPSE_OPS_CONSOLE_SMOKE_LISTEN:-127.0.0.1:18082}"
server_pid=""

cleanup() {
  status=$?
  if [[ $status -ne 0 && -f "$smoke_root/server.log" ]]; then
    cat "$smoke_root/server.log" >&2 || true
  fi
  if [[ -n "$server_pid" ]] && kill -0 "$server_pid" 2>/dev/null; then
    kill "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
  fi
  rm -rf "$smoke_root"
  exit "$status"
}
trap cleanup EXIT

mkdir -p "$runtime_store" "$audit_root" "$node_root" "$download_root"

export SYNAPSE_RUNTIME_STORE_DIR="$runtime_store"
export SYNAPSE_AUDIT_ROOT="$audit_root"
export SYNAPSE_API_TOKENS='[{"token":"ops-token","tenants":["*"]},{"token":"tenant-a-token","tenants":["tenant-a"]}]'

cargo run -p synapse-cli -- runtime import-host --language python --version system --command python3 --activate >/dev/null
cargo run -p synapse-cli -- serve --listen "$listen" >"$smoke_root/server.log" 2>&1 &
server_pid="$!"

for _ in {1..50}; do
  if curl --silent --fail "http://$listen/health" >/dev/null 2>&1; then
    break
  fi
  sleep 0.2
done

curl --silent --fail "http://$listen/health" | grep -Fx "ok" >/dev/null

curl --silent --fail \
  -X POST "http://$listen/execute" \
  -H 'Authorization: Bearer tenant-a-token' \
  -H 'content-type: application/json' \
  -d '{
    "request_id": "ops-console-smoke-success",
    "tenant_id": "tenant-a",
    "language": "python",
    "code": "print(\"ops smoke success\")\n",
    "timeout_ms": 5000,
    "memory_limit_mb": 128
  }' >/dev/null

curl --silent --fail \
  -X POST "http://$listen/execute" \
  -H 'Authorization: Bearer tenant-a-token' \
  -H 'content-type: application/json' \
  -d '{
    "request_id": "ops-console-smoke-timeout",
    "tenant_id": "tenant-a",
    "language": "python",
    "code": "while True:\n    pass\n",
    "timeout_ms": 50,
    "memory_limit_mb": 128
  }' >/dev/null

(
  cd "$node_root"
  npm init -y >/dev/null 2>&1
  npm install puppeteer-core >/dev/null 2>&1
)

SMOKE_URL="http://$listen/admin/console" \
SMOKE_DOWNLOAD_DIR="$download_root" \
SMOKE_NODE_ROOT="$node_root" \
CHROME_BIN="$chrome_bin" \
node <<'NODE'
const fs = require("fs");
const path = require("path");
const puppeteer = require(path.join(process.env.SMOKE_NODE_ROOT, "node_modules/puppeteer-core"));

async function main() {
  const browser = await puppeteer.launch({
    headless: true,
    executablePath: process.env.CHROME_BIN,
    args: ["--no-sandbox"],
  });

  const page = await browser.newPage();
  const response = await page.goto(process.env.SMOKE_URL, {
    waitUntil: "domcontentloaded",
    timeout: 15000,
  });
  if (!response || response.status() !== 200) {
    throw new Error(`expected /admin/console to return 200, got ${response ? response.status() : "no response"}`);
  }

  await page.setViewport({ width: 1440, height: 1100, deviceScaleFactor: 1 });
  await page.type("#token-input", "ops-token");
  await page.type("#tenant-input", "tenant-a");
  await page.click("#refresh-all-button");
  await page.waitForFunction(
    () => document.querySelector("#service-status")?.textContent.includes("Service ok"),
    { timeout: 15000 }
  );

  await page.click('.tab[data-view="requests"]');
  await page.waitForFunction(() => location.hash === "#/requests", { timeout: 10000 });
  await page.waitForSelector("#requests-body tr", { timeout: 10000 });
  const rows = await page.$$eval("#requests-body tr", (items) => items.length);
  if (rows < 2) {
    throw new Error(`expected at least 2 request rows, got ${rows}`);
  }

  await page.evaluate(() => document.querySelector('[data-request-open="ops-console-smoke-timeout"]')?.click());
  await page.waitForFunction(() => location.hash === "#/request/ops-console-smoke-timeout", {
    timeout: 10000,
  });
  await page.waitForFunction(
    () => document.querySelector("#detail-summary")?.innerText.includes("ops-console-smoke-timeout"),
    { timeout: 10000 }
  );
  const auditItems = await page.$$eval("#detail-audit .timeline-item", (items) => items.length);
  if (auditItems === 0) {
    throw new Error("expected audit timeline items for timeout request");
  }

  const client = await page.target().createCDPSession();
  await client.send("Page.setDownloadBehavior", {
    behavior: "allow",
    downloadPath: process.env.SMOKE_DOWNLOAD_DIR,
  });
  await page.click("#download-audit-button");
  await new Promise((resolve) => setTimeout(resolve, 1500));
  const downloaded = path.join(process.env.SMOKE_DOWNLOAD_DIR, "ops-console-smoke-timeout.json");
  if (!fs.existsSync(downloaded)) {
    throw new Error("expected audit download file to exist");
  }

  const mobile = await browser.newPage();
  await mobile.setViewport({
    width: 390,
    height: 844,
    isMobile: true,
    hasTouch: true,
    deviceScaleFactor: 2,
  });
  await mobile.goto(process.env.SMOKE_URL, {
    waitUntil: "domcontentloaded",
    timeout: 15000,
  });

  const mobileLayout = await mobile.evaluate(() => ({
    controlsColumns: getComputedStyle(document.querySelector(".controls")).gridTemplateColumns,
    filtersColumns: getComputedStyle(document.querySelector(".filters")).gridTemplateColumns,
    statusDirection: getComputedStyle(document.querySelector(".status-bar")).flexDirection,
  }));

  if (mobileLayout.statusDirection !== "column") {
    throw new Error(`expected mobile status bar to stack, got ${mobileLayout.statusDirection}`);
  }

  console.log("ops console smoke passed");
  console.log(JSON.stringify({ rows, auditItems, mobileLayout }, null, 2));

  await mobile.close();
  await browser.close();
}

main().catch((error) => {
  console.error(error.stack || String(error));
  process.exit(1);
});
NODE
