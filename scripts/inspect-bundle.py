"""Verify an NSIS bundle extracted with 7-Zip against the release binaries."""
import argparse
import hashlib
import json
import re
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--extracted", type=Path, required=True)
args = parser.parse_args()
root = Path(__file__).resolve().parent.parent
(root / "docs/logs").mkdir(parents=True, exist_ok=True)
release = root / "target/release"
packed_main = (args.extracted / "codebeat.exe").read_bytes()
built_main = (release / "codebeat.exe").read_bytes()
normalized = packed_main.replace(
    b"TAURI_BUNDLE_TYPE_VAR_NSS", b"TAURI_BUNDLE_TYPE_VAR_UNK"
)
if packed_main != built_main and normalized != built_main:
    raise SystemExit("Unexpected desktop binary difference")
if (args.extracted / "codebeat-runner.exe").read_bytes() != (
    release / "codebeat-runner.exe"
).read_bytes():
    raise SystemExit("Runner binary mismatch")
for name in ("LICENSE", "PRIVACY.md", "THIRD_PARTY.md"):
    if not (args.extracted / name).is_file():
        raise SystemExit(f"Required notice missing: {name}")
if list(args.extracted.rglob("*fixture*.exe")):
    raise SystemExit("Test fixtures must never be distributed")
font_manifest = json.loads((root / "desktop/assets/fonts/source.json").read_text(encoding="utf-8"))
for font in font_manifest["outputs"]:
    if font["file"] in font_manifest["native_faces"]:
        packed = args.extracted / "fonts" / font["file"]
        if hashlib.sha256(packed.read_bytes()).hexdigest() != font["sha256"]:
            raise SystemExit(f"Bundled font mismatch: {font['file']}")
if list((args.extracted / "fonts").glob("*.ttf")):
    raise SystemExit("Desktop and notification windows share embedded WOFF2; TTF resources must not be bundled")
for name in ("OFL.txt", "source.json"):
    if (args.extracted / "fonts" / name).read_bytes() != (root / "desktop/assets/fonts" / name).read_bytes():
        raise SystemExit(f"Bundled font attribution mismatch: {name}")
installer = release / "bundle/nsis/codebeat-Setup-x64.exe"
frontend_assets = re.findall(r'(?:src|href)="(/assets/[^\"]+)"', (root / "desktop/dist/index.html").read_text(encoding="utf-8"))
if not frontend_assets or any(asset.encode() not in packed_main for asset in frontend_assets):
    raise SystemExit("Bundled frontend assets do not match the latest dist build")
result = {
    "installer": installer.name,
    "bytes": installer.stat().st_size,
    "sha256": hashlib.sha256(installer.read_bytes()).hexdigest(),
    "runner_matches": True,
    "main_matches_after_bundle_type_normalization": True,
    "main_difference": "Tauri NSIS bundle type marker UNK -> NSS; all other bytes match.",
    "fixture_binaries_bundled": False,
    "required_notices_present": True,
    "bundled_font_faces_verified": len(font_manifest["native_faces"]),
    "font_license_and_source_manifest_present": True,
    "frontend_assets_current": frontend_assets,
}
(root / "docs/logs/artifact-verification.json").write_text(
    json.dumps(result, indent=2), encoding="utf-8"
)
print(json.dumps(result, indent=2))
