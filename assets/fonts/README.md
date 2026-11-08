# Bundled fonts

All files are under the SIL Open Font License 1.1 (`*-OFL.txt`). Neither family declares a
Reserved Font Name, so the subsets keep their names.

| File | Source | What changed |
|---|---|---|
| `NotoSerifJP-Regular.ttf`, `NotoSerifJP-SemiBold.ttf` | `google/fonts` `ofl/notoserifjp/NotoSerifJP[wght].ttf` | Static instances at weight 400 and 600, subset to Latin, Latin-1, Latin Extended-A, general punctuation and a few symbols. The full font is 13.6 MB; the app only sets Latin headings in it. |
| `Inter-Regular.ttf`, `Inter-Medium.ttf`, `Inter-SemiBold.ttf` | `google/fonts` `ofl/inter/Inter[opsz,wght].ttf` | Static instances at weights 400, 500 and 600 with `opsz` 14, subset to Latin, Greek, Cyrillic, punctuation and common symbols. Text in other scripts falls back to system fonts. |

Both were produced with `fonttools` (`varLib.instancer` then `subset`), hinting dropped.
