# Known issues

What is still open after sections 10.1, 10.3 and 10.5 (PRs #22–#39).

## Editor
- After a capture with redactions made before 10.5 is saved, its original is kept: a flattened block can't be recognised. Won't fix.
- When Windows shuts down or logs off, a full-screen capture whose picture hasn't arrived yet, and a queued Save, Copy or Delete, are lost; queued captures, Back writes and the open editor's edits are written (10.5.12, 10.5.14).
- Removing a stale thumbnail after a redacted edit is best effort: if another program (an antivirus scan) holds the file open at that moment, the old thumbnail stays until it is next replaced (10.5.14 review).

## Screen reader
- Narrator's headings list doesn't show Toru's headings: accesskit_windows, which gpui-pre uses, never sets UI Automation's `HeadingLevel`. Proposed upstream in https://github.com/AccessKit/accesskit/issues/840 with a tested fix; AccessKit takes outside changes as issues only. Once released, `cargo update` brings it in (10.5.15).

## Memory
- After the editor is opened and closed, private bytes stay 12–48 MB higher, allocated outside Toru (most likely the graphics driver); gpui has no call to release it (10.1).

## Installer
- No app icon yet: shortcuts show Windows' default icon.
- Unsigned, so Windows shows a SmartScreen warning on first run.

## Design notes
- The invalid-colour warning border uses one red per look, picked in 10.5.5 (Japandi `#c8553d`, Pixel art `#ff6b97`, Neo-brutalism `#e63946`); they pass the 3:1 contrast check (10.1).
- Charcoal & clay keeps a light toast and bulk bar: the mockup's Linen ink would have no visible edge on its window. Its tags follow the mockup (10.5.16).
- Colours tokens.json fixes for the whole look stay the same in every palette: Pixel art's mint focus ring and sticker yellow, Neo-brutalism's blue focus ring, crosshair and graph-paper canvas, and the annotation pink.

## Needs a person
Real keyboard and mouse use has not been tested: typing on the canvas (AltGr, accent keys, an input method, Ctrl+Z while typing, pasting from another program), resizing and Shift-moving marks, drawing a crop or a blur and saving, Ctrl+Numpad shortcuts, overlay dragging and Esc, tray clicks, Tab, Shift+Tab, arrow keys, Enter, Space and Esc everywhere (10.1), wheel scrolling. Also open: PrtScn with Windows' setting off (1.1); a Narrator listening test of the 10.2 changes and of menus opening from Narrator's activate command (10.5.6); a real Windows shutdown or logoff with the editor open or work queued (10.5.12, 10.5.14); the CJK font order on a Japanese, Korean or Traditional Chinese Windows display language (10.5.13); Windows' Animation effects turned on (10.1, only tested off); and the clean-machine installer checklist in `installer/README.md` (10.3).
