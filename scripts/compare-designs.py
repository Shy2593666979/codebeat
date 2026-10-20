"""Create reproducible visual review artifacts; not a similarity certification."""
from pathlib import Path
import json
from PIL import Image, ImageChops, ImageEnhance

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "docs/screenshots/comparison"
OUT.mkdir(parents=True, exist_ok=True)
references = [
    ("home", "首页.png", (60, 51, 1388, 1049)),
    ("tasks", "任务页.png", (37, 48, 1410, 1067)),
    ("sessions", "会话记录页.png", (51, 42, 1401, 1061)),
    ("usage", "额度页.png", (51, 42, 1400, 1066)),
    ("settings", "设置页.png", (52, 25, 1396, 1071)),
]
sections, records = [], []
for name, filename, bounds in references:
    source = Image.open(ROOT / "docs/images" / filename).convert("RGB").crop(bounds)
    actual = Image.open(ROOT / "docs/screenshots/redesign" / f"{name}.png").convert("RGB")
    source.save(OUT / f"{name}-reference.png")
    # Compare visible client area at 1:1 scale. Full-page capture remains alongside it.
    viewport = actual.crop((0, 0, source.width, source.height))
    viewport.save(OUT / f"{name}-actual.png")
    Image.blend(source, viewport, .5).save(OUT / f"{name}-overlay.png")
    ImageEnhance.Contrast(ImageChops.difference(source, viewport)).enhance(3).save(OUT / f"{name}-difference.png")
    records.append({"page": name, "reference_crop": bounds, "client_size": source.size,
                    "actual_full_page_size": actual.size, "overflow_y": max(0, actual.height-source.height)})
    sections.append(f'<section><h2>{name}</h2><div><figure><figcaption>Reference client area</figcaption><img src="{name}-reference.png"></figure><figure><figcaption>Implemented client area</figcaption><img src="{name}-actual.png"></figure></div><details><summary>Overlay / difference</summary><img src="{name}-overlay.png"><img src="{name}-difference.png"></details></section>')
notification = ROOT / "docs/screenshots/notifications/success.png"
if notification.exists():
    source = Image.open(ROOT / "docs/images/消息通知页.png").convert("RGB").crop((914, 95, 1414, 271))
    actual = Image.open(notification).convert("RGB").crop((12, 12, 512, 188))
    source.save(OUT / "notification-reference.png")
    actual.save(OUT / "notification-actual.png")
    Image.blend(source, actual, .5).save(OUT / "notification-overlay.png")
    ImageEnhance.Contrast(ImageChops.difference(source, actual)).enhance(3).save(OUT / "notification-difference.png")
    sections.append('<section><h2>notification — native WPF render</h2><div><figure><figcaption>Reference card</figcaption><img src="notification-reference.png"></figure><figure><figcaption>Implemented card (real status wording differs)</figcaption><img src="notification-actual.png"></figure></div><details><summary>Overlay / difference</summary><img src="notification-overlay.png"><img src="notification-difference.png"></details></section>')
(OUT / "geometry.json").write_text(json.dumps(records, indent=2), encoding="utf-8")
(OUT / "index.html").write_text('<!doctype html><meta charset="utf-8"><title>CodeBeat visual review</title><style>body{font:16px Segoe UI,sans-serif;background:#eaf3fc;color:#142344;margin:24px}section{margin:32px 0}section>div{display:grid;grid-template-columns:1fr 1fr;gap:16px}figure{margin:0}img{max-width:100%;border:1px solid #aac4df}figcaption{padding:8px}details img{width:49%}</style><h1>CodeBeat reference comparison</h1><p>Left: reference. Right: implementation. Dynamic values differ. Crops are measured manually. No similarity percentage is claimed.</p>'+''.join(sections), encoding="utf-8")
print(json.dumps(records, indent=2))
