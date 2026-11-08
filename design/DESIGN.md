# Snapshot — product & design spec (handoff from prototype)

Working name: **Snapshot**. A cross-platform screenshot app (Windows first, then macOS and Linux).
This document is the source of truth for behaviour. `mockups/` is the source of truth for look.

## 1. Core idea

Captures are **temporary until saved**. Taking a screenshot never writes to disk by itself; it lands in the
app's *session gallery* and (by default) on the clipboard. The user names it, then saves, copies, or ignores it.

## 2. Main window (variant A — chosen)

Layout: title bar → capture toolbar → body (gallery left, settings column right, 300px) → bottom action bar.

**Capture toolbar**: Region (default, primary), Window, Full screen (with display picker), Delayed (3/5/10 s),
shortcut hint, spacer, New session, Settings (gear).

**Gallery**: 3-across grid of cards. Each card = thumbnail (with size · mode · time tag), editable name input,
status chip (`Unsaved` / `Saved ✓`), actions `Copy` + `Save` (unsaved) or `Copy` + `Open folder` (saved), `⋯` menu
(Open in editor, Rename, Reveal, Delete).

- **This session** = everything captured since the window/session started. Default is a clean slate.
- **Show previous screenshots** (toggle, settings column, remembered between launches). When ON, earlier
  *unsaved* captures appear below the session group, grouped by day, newest first.
- **Infinite scroll**: history loads one day-group at a time as the user scrolls (no date filters). A
  loading row ("Loading Tue, Nov 4 …") shows at the bottom while fetching. Thumbnails only; full image loads on open.
- **Back to this session** pill (bottom-right of the gallery) appears only once the session group has scrolled out
  of view; click scrolls to top; disappears when the session group is visible again.
- Search box filters by name. `Select` enters select mode (see §4).

**Settings column**: Session (Show previous), Capture (Copy to clipboard on capture — ON by default; Auto-save every
capture — OFF; Keep running in tray — ON; Open editor after capture — OFF), Shortcuts (PrtScn → Region default,
Ctrl+Shift+W → Window, Ctrl+Shift+F → Full screen, "Edit shortcuts…"), Save to (folder + Change), Name pattern
(`screenshot_{mm}_{dd}_{yyyy}`, same-day duplicates get `_2`, `_3` …; renaming one never renames others), Format
(PNG / JPG / WebP), On close (Ask before discarding unsaved / Keep unsaved for next session / Discard silently).

**Bottom bar**: summary text ("3 captures · 1 saved · 2 unsaved") + `Clear unsaved`, `Copy all`, `Save all (N new)`.
`Save all` skips anything already saved (no duplicates). `Copy all` copies as a set of files (clipboard holds one
image; unsaved shots are written to a temp folder first).

**Feedback**: a 2-second toast ("Copied to clipboard") above the bottom bar; the per-card `Copy` button flips to a
check mark for ~1.5 s then back. Cards never change shape or size for status.

## 3. Tray / shortcut behaviour

- Window can be minimized to the tray; shortcuts keep working. PrtScn runs the default capture (Region) with the
  saved settings and copies to the clipboard. Extra shortcuts can be bound to Window / Full screen / Delayed / a
  specific display.
- Captures taken while minimized still land in the session gallery.

## 4. Select mode (bulk actions)

`Select` button, Ctrl-click a card, or Ctrl+A. Checkboxes appear top-left on every card; Shift-click selects a range.
A dark floating bar replaces the bottom bar while anything is selected:
`N selected · x saved · y unsaved | Copy as files · Copy paths (count of saved only) · Save (N new) · Merge ▾ · Delete · ✕`.

- **Copy paths**: saved items only; one path per line.
- **Delete**: asks once if any selected are unsaved. For saved items it removes the card only, unless
  "also delete files" is ticked.
- **Merge ▾** (optional, v2): stitch selection vertically / horizontally / grid into one new capture.
- Esc or `Done` leaves select mode.

## 5. Editor

Opens from a card's `⋯` → Open in editor, double-click a thumbnail, or "Open editor after capture".
Excalidraw-style floating tool pill, top-centre, grouped: Select (V), Pan (H) | Crop (C), Block/Blur (B) |
Arrow (A), Line (L), Rectangle (R), Ellipse (O) | Draw (P), Highlighter (M), Text (T), Number badge (N),
Stickers (S) | Eraser (E).

- Left context panel changes per tool. Arrow: stroke colour, width, arrow head (→ ↔ ⟶ ↝), style
  (solid/dashed/sketchy), opacity. Text: font/size. Block: Blur / Pixelate / Solid. Stickers: tray with pointing hands
  at 8 angles, check, cross, warning.
- Zoom (−/%/+/Fit) bottom-left; Undo/Redo bottom-right; hint chip bottom-centre.
- Top bar: Back to session, name field, "1440×900 · edited · not saved", Reset edits, Copy, Save (primary), Save as ▾.
- Shift constrains, Esc returns to Select, Ctrl+C copies the edited image. Edits are non-destructive until Save
  (original kept under Reset edits). Back returns to the gallery with the edited version as the card's thumbnail.

## 6. Capture overlay (not yet mocked — implement from these notes)

Dimmed screen; drag to select region with live size readout and magnifier; Window mode highlights the window under
cursor; Esc cancels; Enter/click confirms; after capture show a small quick-rename box for ~2 s (optional).

## 7. Accessibility requirements (non-negotiable)

- All text ≥ 4.5:1 (3:1 for ≥ 24px). Every theme/palette in `tokens.json` was audited on rendered pages.
- 3px focus ring + halo (`--focus`, `--focus-halo`) on every control. Keyboard: Tab through controls, arrow keys move
  between cards (roving tabindex), Enter opens, Space toggles selection in select mode, Esc exits select/editor tool.
- Real `<button>`, `<input>`, `<label>`; icon-only buttons carry `aria-label`; toast and loading row are `aria-live`.
- Min target 32px with 6–8px gaps (44px for primary actions on touch).
- Respect `prefers-reduced-motion` (no slide/fade on toasts). Never dim earlier captures (contrast).
- Never rely on colour alone: status is always text + chip.

## 8. Theming

Seven looks × three palettes live in `tokens.json` (CSS custom properties + fonts + extra CSS + micro-copy).
Everything is themed: window controls, icon stroke style, annotation ink (`--anno`), sticker colour, textures, copy
tone. The app should ship a theme picker (look) and a palette picker (colourway) that swap `:root` vars at runtime.

Pick ONE look as the default before building; the rest are optional themes.

## 9. Suggested stack

The mockups are plain HTML/CSS with CSS variables, so a web-rendered shell ports them directly:
**Tauri 2 (Rust)** — small binaries, native tray, global shortcuts, screen capture via `xcap` / `screenshots` crates;
frontend in Svelte/React/vanilla. Electron is the fallback if a needed capture API is missing.

## 10. Files in this folder

- `mockups/<Style>-<Scene>.html` — 28 high-fidelity mockups (open in any browser). Scenes: Main, Editor, Select, Palettes.
- `wireframes/*.html` — the original grey-box wireframes (Main A/B, Editor, SelectionMode).
- `screenshots/*.png` — rendered PNGs of every mockup.
- `tokens.json` — per-style fonts, CSS vars, extra CSS, copy strings, palettes.
- `build-mockups.py` — the generator; the `STYLES`/`THEME` dicts are the canonical token source.
