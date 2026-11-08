# Bundled fonts

All files are under the SIL Open Font License 1.1 (`*-OFL.txt`). Where a family declares no
Reserved Font Name, its subset keeps the name; Press Start 2P reserves its name, so it ships
unmodified.

| Look | File | Source | What changed |
|---|---|---|---|
| Japandi | `NotoSerifJP-Regular.ttf`, `NotoSerifJP-SemiBold.ttf` | `google/fonts` `ofl/notoserifjp/NotoSerifJP[wght].ttf` | Static instances at weight 400 and 600, subset to Latin, Latin-1, Latin Extended-A, general punctuation and a few symbols. The full font is 13.6 MB; the app only sets Latin headings in it. |
| Japandi | `Inter-Regular.ttf`, `Inter-Medium.ttf`, `Inter-SemiBold.ttf` | `google/fonts` `ofl/inter/Inter[opsz,wght].ttf` | Static instances at weights 400, 500 and 600 with `opsz` 14, subset to Latin, Greek, Cyrillic, punctuation and common symbols. Text in other scripts falls back to system fonts. |
| PixelArt | `PressStart2P-Regular.ttf` | `google/fonts` `ofl/pressstart2p/PressStart2P-Regular.ttf` | Nothing: its OFL reserves the name "Press Start 2P", and the file is 118 KB. |
| PixelArt | `VT323-Regular.ttf` | `google/fonts` `ofl/vt323/VT323-Regular.ttf` | Subset to Latin, Latin-1, Latin Extended-A, general punctuation, arrows, geometric shapes and symbols (153 KB to 65 KB). |
| NeoBrutalism | `ArchivoBlack-Regular.ttf` | `google/fonts` `ofl/archivoblack/ArchivoBlack-Regular.ttf` | The same subset (91 KB to 39 KB), plus the `name` table records below. |
| NeoBrutalism | `SpaceGrotesk-Medium.ttf`, `SpaceGrotesk-Bold.ttf` | `google/fonts` `ofl/spacegrotesk/SpaceGrotesk[wght].ttf` | Static instances at weights 500 and 700, the same subset (137 KB variable to 52 KB each). |

All were produced with `fonttools` (`varLib.instancer` for the variable fonts, then `subset`),
hinting dropped. The subset keeps U+0020–007E, U+00A0–017F, U+2000–206F, U+20AC, U+2190–21FF,
U+2200–22FF, U+25A0–25FF, U+2600–26FF and U+2700–27BF where the font has them. Glyphs a face lacks
(VT323 has no ★ or ✓, for example) fall back to system fonts.

## Archivo Black's `name` table

The source file names the family only in name IDs 1 and 2 ("Archivo Black" / "Regular"). On
Windows, DirectWrite reads the trailing "Black" of that legacy family name as a weight, so the font
registers as family "Archivo" at weight 900 and GPUI's lookup for "Archivo Black" finds nothing;
the text silently falls back to Segoe UI. The typographic names (IDs 16 and 17) and the WWS names
(IDs 21 and 22), which DirectWrite prefers when present, are added so the family is "Archivo Black"
and the style "Regular". Each is a Windows record (platform 3, encoding 1, language 0x409), the
same as the existing IDs 1 and 2. Nothing else in the font changes.

After subsetting, apply them with:

```
python -E -P -c "from fontTools.ttLib import TTFont; f = TTFont('ArchivoBlack-Regular.ttf'); n = f['name']; [n.setName(text, id, 3, 1, 0x409) for id, text in ((16, 'Archivo Black'), (17, 'Regular'), (21, 'Archivo Black'), (22, 'Regular'))]; f.save('ArchivoBlack-Regular.ttf')"
```

The other families keep weight-free family names, so they need no such records.
