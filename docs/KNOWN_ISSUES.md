# Known issues

Left open by the review cap. Fix these in section 10.1.

## Editor text, badges, stickers
- AltGr and dead keys type the wrong thing on the canvas (`editor/mod.rs`, `type_key`).
- Characters the bundled font lacks (CJK, emoji, some Cyrillic) are saved as blanks, with no fallback or warning.
- Resizing can collapse a mark to size 0, leaving it invisible and not resizable.
- Shift-moving a mark flush against an edge can freeze it (`Annotation::moved`).
- Ctrl+Z and Ctrl+Y do nothing while typing on the canvas.
- Changing one option while re-editing text also applies the tool's other options.
- White and Yellow badges make the white digit unreadable.
- A click in the far margin can leave invisible text.
- Text being typed doesn't count as an edit, so a capture can replace the editor and lose it.
- Pasted CRLF leaves `\r` in the text.

## Editor save
- A crop that was dragged but not applied is ignored by Save, Copy, Save as and Back, so the file is uncropped.
- Quitting with the editor open loses unsaved edits.

## Dialogs and shortcuts
- The toast saying a capture went to the gallery is replaced at once by "Copied to clipboard".
- `show_editor` takes focus even when a dialog is open, so Esc and Tab miss the dialog.
- Ctrl+Numpad digits record as the top-row digit, and don't fire.
- The custom palette panel only stops Esc, so Ctrl+A selects the gallery behind it.
- An invalid hex field's border colour has no visible effect.
- Cancelling the quit dialog over the custom palette editor sends focus behind it.

## Decided by the owner, to build in 10.1
- Clear unsaved asks for confirmation first.
- "Also delete files" sends files to the Recycle Bin.
- When a capture with blurred or pixelated areas is saved, remove its kept original, so the hidden content is gone. Reset no longer works on those.

## Waiting on the owner
- Installer tool for 10.3: Inno Setup is proposed. Unsigned, so Windows shows a SmartScreen warning.

## Needs a person
Real keyboard and mouse use has not been tested: overlay dragging and Esc, tray clicks, arrow keys and Tab, wheel scrolling, drawing on the canvas, recording a shortcut. PrtScn with Windows' setting off (1.1) and a Narrator listening test (1.5) are also open.
