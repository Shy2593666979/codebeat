"""Build private, static UI faces from the pinned OFL Noto Sans SC source.

Only needed to regenerate committed assets. Uses .build-tools/font-tools so the
application and regular CI do not need Python font tooling or a font download.
"""
from pathlib import Path
import hashlib
import json
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / '.build-tools/font-tools'))
import fontTools
from fontTools.ttLib import TTFont
from fontTools.ttLib import woff2
from fontTools.varLib.instancer import instantiateVariableFont

# WOFF2 remains lossless. Quality 9 avoids multi-minute maximum-compression passes
# over full CJK glyph sets while keeping a practical application download size.
_compress = woff2.brotli.compress
woff2.brotli.compress = lambda data, **kwargs: _compress(data, quality=9, **kwargs)

out = ROOT / 'desktop/assets/fonts'
source = ROOT / '.build-tools/font-source/NotoSansSC-VF.ttf'
manifest = json.loads((out / 'source.json').read_text(encoding='utf-8-sig'))
if hashlib.sha256(source.read_bytes()).hexdigest() != manifest['sha256']:
    raise SystemExit('Font source SHA-256 mismatch')
manifest['family'] = 'CodeBeat UI'
manifest['derived_from'] = 'Noto Sans SC'
manifest['fonttools_version'] = fontTools.__version__
manifest['woff2_brotli_quality'] = 9
manifest['outputs'] = []
manifest['native_faces'] = []
for weight, style in [(400, 'Regular'), (500, 'Medium'), (600, 'SemiBold'), (700, 'Bold')]:
    font = TTFont(source, recalcTimestamp=False)
    instantiateVariableFont(font, {'wght': weight}, inplace=True)
    names = {1: 'CodeBeat UI', 2: style, 3: 'CodeBeatUI-' + style,
             4: 'CodeBeat UI ' + style, 6: 'CodeBeatUI-' + style,
             16: 'CodeBeat UI', 17: style}
    for record in font['name'].names:
        if record.nameID in names:
            record.string = names[record.nameID].encode(record.getEncoding())
    for name_id, value in names.items():
        font['name'].setName(value, name_id, 3, 1, 0x409)
    font['OS/2'].usWeightClass = weight
    font['OS/2'].fsSelection &= ~(1 << 5 | 1 << 6)
    font['head'].macStyle &= ~1
    if weight == 700:
        font['OS/2'].fsSelection |= 1 << 5
        font['head'].macStyle |= 1
    elif weight == 400:
        font['OS/2'].fsSelection |= 1 << 6
    file = out / f'CodeBeatUI-{style}.ttf'
    font.save(file)
    font.flavor = 'woff2'
    web = file.with_suffix('.woff2')
    font.save(web)
    for output in [file, web]:
        manifest['outputs'].append({'file': output.name, 'weight': weight,
                                   'bytes': output.stat().st_size,
                                   'sha256': hashlib.sha256(output.read_bytes()).hexdigest()})
    print(f'{weight} {style}: {file.stat().st_size} TTF bytes, {web.stat().st_size} WOFF2 bytes', flush=True)
(out / 'source.json').write_text(json.dumps(manifest, indent=2), encoding='utf-8')
