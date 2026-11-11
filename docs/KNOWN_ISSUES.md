# Known issues

Left open by the review cap, or by decisions still to make. Section 10.5 (PRs #22–#34) fixed the earlier list.

## Editor
- After a capture with redactions made before 10.5 is saved, its original is kept: a flattened block can't be recognised. Won't fix.
- When Windows shuts down or logs off, a capture or a Back write still waiting on the capture queue is lost; only the open editor's edits are written then (10.5.12).
- If a redacted Save's card-picture write fails after the picture but before its thumbnail, the editor isn't told what the card shows; the card still holds only the redacted picture (PR #32 review).

## Screen reader
- Narrator's headings list may not show the gallery headings; they are still read as text.

## Decisions for the owner
- The invalid-colour warning border uses one red per look, picked in 10.5.5 (Japandi `#c8553d`, Pixel art `#ff6b97`, Neo-brutalism `#e63946`); the mockups have no warning colour.
- Charcoal & clay's toast, bulk bar and thumbnail tags now follow its own colours (light toast and bar, dark tags); the Palettes mockup draws them in Linen's colours.
- Colours tokens.json fixes for the whole look stay the same in every palette: Pixel art's mint focus ring and sticker yellow, Neo-brutalism's blue focus ring, crosshair and graph-paper canvas, and the annotation pink.
- Installer tool for 10.3: Inno Setup is proposed. Unsigned, so Windows shows a SmartScreen warning.

## Needs a person
Real keyboard and mouse use has not been tested: typing on the canvas (AltGr, accent keys, an input method, Ctrl+Z while typing, pasting from another program), resizing and Shift-moving marks, drawing a crop or a blur and saving, Ctrl+Numpad shortcuts, overlay dragging and Esc, tray clicks, arrow keys and Tab, wheel scrolling. PrtScn with Windows' setting off (1.1) is also open, and so is a Narrator listening test of the 10.2 changes and of menus opening from Narrator's activate command (10.5.6). Also untested: a real Windows shutdown or logoff with the editor open (10.5.12), and the CJK font order on a Japanese, Korean or Traditional Chinese Windows display language (10.5.13).
