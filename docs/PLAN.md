# Toru build plan

Each section below is one reviewed pull request, squash-merged to `main` (see the working rules in `CLAUDE.md`).
**[Fable]** marks sections that are mostly visual. New UI in any other section, such as a dialog, still goes to Fable.
Sections may run at the same time in separate worktrees.

Sections name the main items, not every control. `design/DESIGN.md` plus the overrides in `CLAUDE.md` are the full
spec: Phase 5 onward, a section isn't done until every control it touches works with real data as they describe.

## Phase 0: Foundation
- [x] 0.1 Set up `main` and HTTPS, add `design/`, `CLAUDE.md` and this plan
- [x] 0.2 Pin the toolchain and open a blank GPUI window

## Phase 1: Prove the risky parts
Results go in `docs/spikes.md`.
- [ ] 1.1 Tray and global hotkeys alongside GPUI, and PrtScn with Snipping Tool's binding on and off
- [x] 1.2 Capture overlay: a borderless topmost window per monitor showing a frozen image
- [x] 1.3 Name fields with real text editing, and smooth scrolling with 200 real thumbnails
- [x] 1.4 Copy image and copy as files
- [ ] 1.5 Narrator check (Windows UI Automation passed; the owner's listening test in `docs/spikes.md` is still open)

## Phase 2: Core logic, no UI
- [x] 2.1 Capture records and naming with `_2`/`_3` **[parallel with 2.3]**
- [x] 2.2 Temp storage and SQLite history
- [x] 2.3 Saved settings **[parallel with 2.1]**

## Phase 3: Look and controls [Fable]
- [x] 3.1 Theme loader, bundled fonts, and Japandi with its 3 palettes
- [x] 3.2 Shared controls: button, toggle, segmented control, chip, input, menu, toast, with focus rings
- [x] 3.3 PixelArt and NeoBrutalism with their palettes, plus an automated contrast test

## Phase 4: Main window with mock data [Fable]
- [x] 4.1 Title bar and toolbar, with the display picker and expanded Delayed menu
- [x] 4.2 Gallery grid, cards and day groups with Collapse
- [x] 4.3 Settings column with collapse
- [x] 4.4 Bottom bar, toast, top-centre return pill, search, keyboard navigation
- [x] 4.5 Select mode and bulk bar

## Phase 5: Real capture
The local date and time come from `GetLocalTime` in `crates/toru/src/platform.rs`, so no date library is needed.
- [x] 5.1 Full screen capture, copy on capture, real cards in the gallery
- [x] 5.2 Region overlay with size readout and magnifier **[Fable for visuals]**
- [x] 5.3 Window capture, with the overlay highlighting the window under the cursor **[Fable for visuals]**
- [x] 5.4 Delayed capture with mode choice
- [x] 5.5 Card Copy and Save, Save all, Copy all, Clear unsaved, Open folder, formats, name pattern, rename, auto-save
- [x] 5.6 Bulk actions, card menu (Rename, Reveal, Delete), New session, changing the Save to folder

## Phase 6: Tray and shortcuts
- [x] 6.1 Tray, keep running, On close options, and a second launch showing the running app
- [x] 6.2 Rebindable shortcuts, including per-display shortcuts and their setting, and conflict messages **[Fable for the dialog]**

## Phase 7: History
- [x] 7.1 Load a day at a time, plus the midnight reset
- [x] 7.2 "Delete unsaved after" setting and cleanup

## Phase 8: Editor [Fable]
Drawn with `tiny-skia`, so the screen matches the saved file exactly.
- [x] 8.1 Opening the editor (`⋯` menu, double-click, Open editor after capture), canvas, zoom, pan, undo, top bar,
  tool pill and context panel. Each later section adds its tools' options to the panel.
- [x] 8.2 Arrow, line, rectangle, ellipse, pen, highlighter, and the Select tool to move and resize them
- [x] 8.3 Text, number badges, stickers
- [x] 8.4 Crop, blur/pixelate/solid, eraser
- [x] 8.5 Save, Save as, Copy, Reset, updated thumbnail

## Phase 9: Appearance [Fable]
- [x] 9.1 Look and palette pickers in settings
- [x] 9.2 Custom fourth palette and background photo, with a contrast warning

## Phase 10: Finish and ship
- [ ] 10.1 Keyboard, contrast and reduced-motion pass, plus idle memory
- [ ] 10.2 Screen reader pass, if 1.5 worked
- [ ] 10.3 Installer that ships the font licences, start with Windows, clean-machine test
- [ ] 10.4 Layout and looks fixes from the Nov 9 review: columns that follow the window, empty state, select bar, bottom bar [Fable]

## Approved libraries
| Job | Library |
|---|---|
| UI | `gpui-pre` 0.3.11 (pinned exactly) |
| Text input, scrolling list | `gpui-kit` 0.9.0, only these parts |
| Capture | `xcap` 0.9.8 |
| Tray | `tray-icon` 0.26.1 |
| Global shortcuts | `global-hotkey` 0.9.0 |
| Clipboard with file lists | `clipboard-rs` 0.3.5 |
| Encoding | `image` 0.25, `webp` 0.3 |
| Editor drawing | `tiny-skia` 0.12 |
| Editor text outlines | `ttf-parser` 0.25 (already used by GPUI) |
| History | `rusqlite` 0.41.1 |
| Folder picker | `rfd` 0.18 |

## Test setup
Two monitors, same resolution, different refresh rates. Mixed display scaling is not covered by the owner's setup.
