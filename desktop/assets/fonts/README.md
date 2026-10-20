# CodeBeat UI

Private application font family derived from Noto Sans SC, distributed under
SIL Open Font License 1.1. Copyright and license: [OFL.txt](OFL.txt).

The source revision, source SHA-256, conversion tool version and output hashes
are in [source.json](source.json). The full source character repertoire is
preserved. Four static faces (400/500/600/700) use the `CodeBeat UI` family.
The desktop UI and independent Tauri notification windows share four embedded
WOFF2 files. TTF files are regeneration intermediates and are not distributed;
`native_faces` is empty. No system font installation or runtime download occurs.

To regenerate, install FontTools and Brotli into `.build-tools/font-tools`,
download the pinned source URL in source.json to
`.build-tools/font-source/NotoSansSC-VF.ttf`, then run
`python scripts/build-fonts.py`. Normal app builds use the committed outputs.
