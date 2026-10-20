"""Verify immutable logo source and actual Windows executable icon resources."""
import base64
import ctypes as c
import hashlib
import json
from pathlib import Path
import re
import struct

root = Path(__file__).resolve().parents[1]
(root / "docs/logs").mkdir(parents=True, exist_ok=True)
manifest = json.loads((root / 'desktop/assets/brand/manifest.json').read_text(encoding='utf-8'))
for item in manifest['files']:
    assert hashlib.sha256((root / item['path']).read_bytes()).hexdigest() == item['sha256'], item['path']
source = root / manifest['source']
if source.suffix.lower() == '.svg':
    assert (root / 'desktop/assets/brand/app-icon.svg').read_bytes() == source.read_bytes(), 'Canonical SVG altered'
    assert (root / 'desktop/src/assets/app-logo.svg').read_bytes() == source.read_bytes(), 'Frontend SVG altered'
    if manifest.get('native_source'):
        native = (root / manifest['native_source']).read_text(encoding='utf-8')
        viewbox = ' '.join(map(str, manifest['native_viewbox']))
        assert native.replace(f'viewBox="{viewbox}"', 'viewBox="0 0 1024 1024"', 1) == source.read_text(encoding='utf-8'), 'Native paths or color altered'
else:
    embedded = re.search(r'base64,([^"\s]+)', (root / 'desktop/assets/brand/app-icon.svg').read_text()).group(1)
    assert base64.b64decode(embedded) == source.read_bytes(), 'Source logo altered'

ico = (root / 'desktop/src-tauri/icons/icon.ico').read_bytes()
count = struct.unpack_from('<H', ico, 4)[0]
expected = []
for index in range(count):
    *_, length, offset = struct.unpack_from('<BBBBHHII', ico, 6 + index * 16)
    expected.append(hashlib.sha256(ico[offset:offset + length]).hexdigest())

kernel = c.WinDLL('kernel32', use_last_error=True)
kernel.LoadLibraryExW.argtypes = [c.c_wchar_p, c.c_void_p, c.c_uint32]
kernel.LoadLibraryExW.restype = c.c_void_p
kernel.FindResourceW.argtypes = [c.c_void_p, c.c_void_p, c.c_void_p]
kernel.FindResourceW.restype = c.c_void_p
kernel.LoadResource.argtypes = [c.c_void_p, c.c_void_p]
kernel.LoadResource.restype = c.c_void_p
kernel.LockResource.argtypes = [c.c_void_p]
kernel.LockResource.restype = c.c_void_p
kernel.SizeofResource.argtypes = [c.c_void_p, c.c_void_p]
kernel.SizeofResource.restype = c.c_uint32
kernel.FreeLibrary.argtypes = [c.c_void_p]
callback_type = c.WINFUNCTYPE(c.c_bool, c.c_void_p, c.c_void_p, c.c_void_p, c.c_ssize_t)
kernel.EnumResourceNamesW.argtypes = [c.c_void_p, c.c_void_p, callback_type, c.c_ssize_t]


def resource(library, kind, name):
    found = kernel.FindResourceW(library, name, kind)
    if not found:
        raise OSError(c.get_last_error(), 'Missing Windows icon resource')
    return c.string_at(kernel.LockResource(kernel.LoadResource(library, found)), kernel.SizeofResource(library, found))


def inspect(path):
    library = kernel.LoadLibraryExW(str(path), None, 2)  # LOAD_LIBRARY_AS_DATAFILE: never execute code
    if not library:
        raise OSError(c.get_last_error(), str(path))
    groups, errors = [], []

    @callback_type
    def collect(handle, kind, name, unused):
        try:
            group = resource(handle, 14, name)
            hashes = []
            for index in range(struct.unpack_from('<H', group, 4)[0]):
                icon_id = struct.unpack_from('<H', group, 6 + 14 * index + 12)[0]
                hashes.append(hashlib.sha256(resource(handle, 3, icon_id)).hexdigest())
            groups.append(hashes)
            return True
        except Exception as error:
            errors.append(str(error))
            return False

    try:
        kernel.EnumResourceNamesW(library, 14, collect, 0)
        assert not errors, errors
        assert any(sorted(group) == sorted(expected) for group in groups), f'Icon group mismatch: {path.name}'
        return {'file': str(path.relative_to(root)).replace('\\', '/'), 'new_icon_frames_verified': count}
    finally:
        kernel.FreeLibrary(library)


result = {'source_unchanged': True, 'manifest_files_verified': len(manifest['files']), 'resources': [
    inspect(root / 'target/release/codebeat.exe'),
    inspect(root / 'target/release/bundle/nsis/codebeat-Setup-x64.exe'),
]}
(root / 'docs/logs/logo-resource-verification.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
print(json.dumps(result, indent=2))
