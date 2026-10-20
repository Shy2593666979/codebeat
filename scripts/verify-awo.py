"""Local AWO regression checks. Uses fixture executables, never live model calls."""
import json
import os
from pathlib import Path
import subprocess
import time

root = Path(__file__).resolve().parents[1]
logs = root / "docs" / "logs"
logs.mkdir(parents=True, exist_ok=True)
npm = "npm.cmd" if os.name == "nt" else "npm"
checks = [
    ("frontend-build", [npm, "--prefix", "desktop", "run", "build"]),
    ("frontend-tests", [npm, "--prefix", "desktop", "test"]),
    ("rust-tests", ["cargo", "test", "--workspace", "--no-fail-fast"]),
    ("format", ["cargo", "fmt", "--all", "--check"]),
    ("clippy", ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"]),
    ("ui-tests", [npm, "--prefix", "desktop", "run", "test:ui", "--", "--workers=2"]),
]
results = []
for name, command in checks:
    started = time.monotonic()
    with (logs / f"awo-{name}.txt").open("w", encoding="utf-8") as output:
        result = subprocess.run(command, cwd=root, stdout=output, stderr=subprocess.STDOUT)
    entry = {"check": name, "exit_code": result.returncode, "seconds": round(time.monotonic() - started, 2)}
    results.append(entry)
    print(json.dumps(entry), flush=True)
    if result.returncode:
        print((logs / f"awo-{name}.txt").read_text(encoding="utf-8", errors="replace")[-12000:], flush=True)
        break
report = {"checks": results, "live_model_requests": False, "reset_cards_consumed": False,
          "physical_sleep_wake_tested": False, "packaged": False,
          "passed": len(results) == len(checks) and all(r["exit_code"] == 0 for r in results)}
(logs / "awo-validation.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
raise SystemExit(0 if report["passed"] else 1)
