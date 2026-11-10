# Known issues

Left open by the review cap, or by decisions still to make. Section 10.5 (PRs #22–#31) fixed the earlier list.

## Editor
- A redacted editor Save whose file write fails after the card's picture was stored doesn't tell the editor or the gallery: the card shows the blurred picture only after a reload, and an Undo then Back doesn't write the undone state (PR #29 round 3).
- Quitting with the editor open still loses its edits when Windows shuts down or logs off; there's no time to write the picture then.
- If writing the edits fails when quitting and the user then cancels the quit, a later Back doesn't retry the write.
- After a capture with redactions made before 10.5 is saved, its original is kept: a flattened block can't be recognised.
- Text fallback fonts are loaded on the UI thread the first time a script needs them; a character no listed font has reads all seven files once (about 68 MB, kept in memory).

## Screen reader
- Narrator's headings list may not show the gallery headings; they are still read as text.

## Decisions for the owner
- Japanese text uses Microsoft YaHei for kanji, so it gets the Simplified Chinese forms (fallback order in `platform::fallback_fonts`).
- The invalid-colour warning border uses one red per look, picked in 10.5.5 (Japandi `#c8553d`, Pixel art `#ff6b97`, Neo-brutalism `#e63946`); the mockups have no warning colour.
- Charcoal & clay's toast, bulk bar and thumbnail tags now follow its own colours (light toast and bar, dark tags); the Palettes mockup draws them in Linen's colours.
- Colours tokens.json fixes for the whole look stay the same in every palette: Pixel art's mint focus ring and sticker yellow, Neo-brutalism's blue focus ring, crosshair and graph-paper canvas, and the annotation pink.
- Installer tool for 10.3: Inno Setup is proposed. Unsigned, so Windows shows a SmartScreen warning.

## Needs a person
Real keyboard and mouse use has not been tested: typing on the canvas (AltGr, accent keys, an input method, Ctrl+Z while typing, pasting from another program), resizing and Shift-moving marks, drawing a crop or a blur and saving, Ctrl+Numpad shortcuts, overlay dragging and Esc, tray clicks, arrow keys and Tab, wheel scrolling. PrtScn with Windows' setting off (1.1) is also open, and so is a Narrator listening test of the 10.2 changes and of menus opening from Narrator's activate command (10.5.6).
