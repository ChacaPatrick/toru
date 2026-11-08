# Spike results

## 1.1 Tray and global hotkeys

**Answer:** `tray-icon` 0.26.1 and `global-hotkey` 0.9.0 work alongside GPUI's event loop with no extra thread or
message pump. PrintScreen, Ctrl+Shift+W and Ctrl+Shift+F all reach Toru when pressed, and while Toru holds PrintScreen
Snipping Tool doesn't open. Those presses were synthesized with `SendInput`. Real clicks on the tray icon are still
unverified, because the icon sits in the hidden-icons overflow.

Tested 2026-11-08 on Windows 11 build 26200, Rust 1.98.1, `gpui-pre` 0.3.11. Code: `crates/toru/examples/spike_tray.rs`.

### What was tested

The example opens a GPUI window, adds a tray icon with an Open window / Quit menu, and registers the hotkeys. Every
received event is printed to stdout and added to the window's event list.

| Part | Result | How it was checked |
|---|---|---|
| Register PrintScreen, no modifiers | Succeeded | Stdout. `PrintScreenKeyForSnippingEnabled` was absent from `HKCU\Control Panel\Keyboard` |
| Register Ctrl+Shift+W, Ctrl+Shift+F | Succeeded | Stdout |
| Hotkey delivery | Works | Synthesized Ctrl+Shift+F12 (a spike-only hotkey), twice. `Pressed` and `Released` both reached GPUI |
| Tray event delivery | Works | Posted left-button down/up to the tray's hidden window. Two `Click` events reached GPUI |
| Tray icon known to the shell | Yes | Those events carry a real icon rectangle from `Shell_NotifyIconGetRect` |
| Menu event delivery | Works | Posted `WM_COMMAND` for each item to the tray's hidden window |
| Close window, app keeps running | Works | Posted `WM_CLOSE`. Process stayed alive, tray and hotkey windows stayed |
| Tray Open reopens the window | Works | New `Zed::Window` appeared with the event history intact, and hotkeys still worked |
| Tray Quit exits | Works | Exit code 0, stderr empty |
| GPUI view updates from events | Works | The view's `render` ran again after events arrived |

### The approach that worked

- **Quit mode:** `application().with_quit_mode(QuitMode::Explicit)`. The default ends the app when the last window
  closes. Quit calls `cx.quit()`.
- **Thread:** create `GlobalHotKeyManager` and `TrayIcon` on GPUI's main thread, inside the `run` callback. Each owns a
  hidden window (`global_hotkey_app`, `tray_icon_app`).
- **Message pump:** none needed. GPUI's `GetMessageW(&mut msg, None, 0, 0)` loop takes messages for every window on the
  thread, so it also dispatches `WM_HOTKEY` and the tray's messages.
- **Reaching GPUI:** use each crate's `set_event_handler` (`GlobalHotKeyEvent`, `TrayIconEvent`, `MenuEvent`) to send
  into one `std::sync::mpsc` channel. A `cx.spawn` task on the main thread loops on
  `cx.background_spawn(async move { let m = receiver.recv(); (receiver, m) }).await` and handles each message in
  `cx.update`. No polling, and all events were handled on the main thread (`ThreadId(1)`).
- **Handlers must not touch GPUI.** Most run inside a window procedure on the main thread, and hotkey `Released` runs on
  a short-lived thread `global-hotkey` spawns for each press (`ThreadId(4)` and `(6)` in the run).
- **Lifetime:** the `spawn` task owns the manager and tray icon. When it ends on Quit, dropping the tray icon removes it
  from the tray.
- **Reopening:** GPUI's `App::hide` does nothing on Windows, so closing the window is the way to get to the tray. Keep
  state in an entity outside the window and open a new window to show it again, as the example does.

### Limits

- Waiting on the channel holds one Windows thread-pool thread blocked for the life of the app.
- While registered, a hotkey is taken from every other app: Ctrl+Shift+W and Ctrl+Shift+F won't reach a focused browser
  or editor. Registration fails if another app already holds the combination.
- `global-hotkey` sets `MOD_NOREPEAT`, so holding a key sends one `Pressed`.
- Killing the process rather than using Quit can leave a stale icon in the tray until the pointer passes over it.

### Hands-on checks

Run 2026-11-08 by an agent on the owner's desktop, with no person watching. Keys, clicks and pointer moves were
synthesized with `SendInput`. The tray icon is in the hidden-icons overflow: `Shell_NotifyIconGetRect` returns
(2321, 1392, 32x48), which is the "Show Hidden Icons" chevron. Opening that flyout means clicking Explorer, which the
test limits ruled out, so the popup menu was opened by posting `WM_USER_TRAYICON` to Toru's own hidden tray window. It
is the real popup menu, and its items were clicked with real input. Window contents were checked from crops of Toru's
own window, saved to the temp folder, viewed and deleted.

The value `PrintScreenKeyForSnippingEnabled` was set without a sign-out or Explorer restart, so the shell may not have
re-read it. A control press with the value at 1 and Toru not running opened "Snipping Tool Overlay", dismissed with
Esc. No control press was made at 0, because with the setting really off PrintScreen copies the whole screen to the
clipboard. Whether 0 took effect is therefore unknown, but the result with Toru running matched at every value. The
value was absent before testing and was deleted again afterwards.

### Not verified

- Real mouse clicks on the tray icon, because it's in the overflow flyout. The menu's opening was posted, as above.
- How the icon and its tooltip look. Neither was visible.
- The icon leaving the tray right after Quit. The process ended with exit code 0 and empty stderr, but the icon was
  hidden throughout.
- GPUI liveness in the modal loop was checked only by a hotkey line arriving, not by anyone watching the window.
- Whether a value of 0 for the PrintScreen setting took effect without an Explorer restart.
- Recovery after Explorer restarts, which `tray-icon` handles through `TaskbarCreated`.

### Owner's manual checklist

Run from PowerShell, with `%USERPROFILE%\.cargo\bin` on `PATH`: `cargo run -p toru --example spike_tray`. The setting is
in Settings > Accessibility > Keyboard, "Use the Print screen key to open screen capture". Ticked items passed in the
checks above with synthesized input; the unticked ones still need a person.

Tray and window:
- [ ] The orange square appears in the tray, with the tooltip "Toru tray spike". Not seen: the icon is in the overflow.
- [x] The window lists the four registration results and each event as it arrives. Crop of the window inspected.
- [ ] Left-click the icon: a `tray Click` line appears and no menu opens. Only with a posted message: the line
  appeared and no menu opened.
- [ ] Right-click the icon: the menu opens. Press a hotkey with it open; the line appears in the window. Menu opened by
  a posted message; a real Ctrl+Shift+F12 then logged `Pressed` and `Released` with the menu still open.
- [x] Press Esc or click elsewhere: the menu closes. Real Esc and a real click in Toru's window each closed it.
- [x] With the window open but behind another app, choose Open window: it comes to the front. Real click on the menu
  item, with Toru behind a throwaway window from the test: Toru became foreground and was above it in z-order.
- [x] Close the window with its X: the app keeps running in the tray and the hotkeys still print to the console. Real
  click on `HTCLOSE`; Ctrl+Shift+W still logged both lines.
- [x] Choose Open window: the window returns, in front, with the earlier lines. Foreground, and a crop showed the lines.
- [ ] Choose Quit: the process ends and the icon leaves the tray straight away. Process ended with code 0 from a real
  click on Quit; the icon leaving wasn't observable.

Hotkeys, in a window that doesn't use these shortcuts:
- [x] Press Ctrl+Shift+W: `Pressed` then `Released` appear. Real synthesized key press.
- [x] Press Ctrl+Shift+F: `Pressed` then `Released` appear. Real synthesized key press.

PrintScreen, restarting the example after each change of the setting. Registry value set without a restart:

| Setting | Registration line | Toru gets `Pressed` and `Released`? | Snipping Tool opens? |
|---|---|---|---|
| Absent (default) | `PrintScreen: registered` | Yes | No |
| On (value 1) | `PrintScreen: registered` | Yes | No; it did open in the control press with Toru not running |
| Off (value 0) | `PrintScreen: registered` | Yes | No; whether 0 took effect is unknown |

Clipboard sequence number didn't change on any press with Toru running. Registering the key takes precedence over the
shell's own handling in every state observed here. The Off row is not established, so section 1.1 stays open until:

- [ ] Owner: turn the setting off in Settings, restart the example, press PrintScreen, and note whether Toru logs
  `Pressed` and whether the screen is also copied to the clipboard.

## 1.2 Capture overlay

**Answer:** the plan works for capture and window placement. Both monitors are captured in 64–72 ms, both windows exist
224–266 ms after capture starts, and GPUI can put a borderless topmost window on each one at exactly the monitor's
physical bounds. The frozen image is shown in the right place. A GDI recapture of the overlay doesn't match it pixel for pixel: 3–9% of
pixels differ by more than 3 levels, and this isn't noise over time. The difference is on edges only and happens somewhere
between GPUI's drawing and the readback; its cause is unknown, see "Pixel mismatch" below. Crops match the captured frame, since they come from
it, but whether that frame is itself faithful to the screen is unverified. Real drags, Esc and taskbar coverage now pass with synthesized input; what still needs a person
is in the checklist below.

Tested 2026-11-08 on Windows 11 build 26200, Rust 1.98.1, `gpui-pre` 0.3.11, `xcap` 0.9.8, release build. Code:
`crates/toru/examples/spike_overlay.rs`.

### What was tested

Three runs, each with `TORU_SPIKE_AUTOCLOSE_MS=1000`. The overlay closed after about 1.05 s, then stayed on screen while
the pixel comparison ran, because it blocks the main thread (see "Closing" below). All three
exited with code 0 and empty stderr. No mouse or key input was synthesized. In auto-close mode the example sets a fixed
selection in code, then captures each monitor again in memory just before closing and compares that against what the
overlay should be showing. It prints only counts and positions, and nothing is written to disk. Run 3 also forced every
pixel opaque to test the alpha theory below; that line changed nothing and was removed, leaving only the count.

Monitors: primary 2560x1440 at 240 Hz at (0, 0), second 2560x1440 at 60 Hz at (8, -1440), both at scale 1.

| Measure | Run 1 | Run 2 | Run 3 |
|---|---|---|---|
| List monitors | 243 µs | 29 µs | 24 µs |
| Capture primary | 38.3 ms | 32.8 ms | 31.4 ms |
| Capture second | 27.4 ms | 25.5 ms | 26.2 ms |
| **Capture total** | **72.5 ms** | **64.0 ms** | **67.4 ms** |
| First render, primary | 231 ms | 200 ms | 193 ms |
| First render, second | 258 ms | 228 ms | 219 ms |
| Both windows opened | 266 ms | 234 ms | 224 ms |

All times are from the start of capture. "First render" is GPUI building the frame, not the moment it reached the
screen, which wasn't measured. Recapturing a monitor took 22–25 ms.

| Part | Result |
|---|---|
| Window bounds vs monitor, primary | (0, 0, 2560, 1440) vs (0, 0, 2560, 1440), scale 1, match, all 3 runs |
| Window bounds vs monitor, second | (8, -1440, 2560, 1440) vs (8, -1440, 2560, 1440), scale 1, match, all 3 runs |
| Fixed selection, physical size | 400x200 selected, 400x200 cropped, on both monitors, all 3 runs |
| Pixels in the frame that weren't opaque | 0 on both monitors (run 3) |

The bounds are GPUI's `window.bounds()`, which come from `WM_MOVE` and `WM_SIZE`, so they're reported by the OS for the
client area. `xcap` skips the current process's windows, so it couldn't provide a second opinion, and `GetWindowRect`
wasn't called.

Pixel comparison, run 3, against the original frame (dimmed area expected at 50%, selection expected unchanged). The
readout is excluded. "Wrong" means a channel is off by more than 3 levels:

| Zone | Pixels | Off by 0 | Off by 1 or less | Wrong | Worst |
|---|---|---|---|---|---|
| Primary, dimmed | 3,603,520 | 89.6% | 95.0% | 109,990 (3.1%) | 80 |
| Primary, selection | 80,000 | 59.1% | 80.4% | 4,314 (5.4%) | 93 |
| Second, dimmed | 3,603,520 | 77.8% | 90.7% | 203,772 (5.7%) | 79 |
| Second, selection | 80,000 | 60.9% | 79.9% | 7,508 (9.4%) | 68 |

Wrong pixels with the frame shifted by one pixel, to check alignment:

| Shift | Primary | Second |
|---|---|---|
| None | 114,304 | 211,280 |
| Right | 191,567 | 302,278 |
| Left | 191,940 | 301,936 |
| Down | 204,828 | 331,921 |
| Up | 201,489 | 324,352 |

No shift beats the unshifted image, so there's no whole-pixel misalignment. The wrong pixels span the whole of both
monitors, from (1, 1) to (2558, 1438), so it isn't a window or the taskbar sitting on top. Alpha is ruled out, since
every pixel was opaque, and the readout is excluded from the counts.

### Pixel mismatch

Measured 2026-11-08 with temporary diagnostics, since removed, and a throwaway tool outside the repo. Nothing was
written to disk. Same metric as above: a pixel is wrong if a channel is off by more than 3 levels.

| Baseline, no overlay: two in-memory `xcap` captures | Primary | Second |
|---|---|---|
| Back to back | 0 wrong (worst 1) | 0 wrong (worst 0) |
| 1 s apart | 1,282 (0.035%) | 0 |
| 3 s apart | 10,713 (0.291%) | 0 |

The baseline differences were confined to a few grid cells of live content. Two recaptures 300 ms apart under the
overlay were identical on both monitors, so GDI readback under the overlay is stable. With the overlay up, the wrong
counts were: primary dimmed 152,377 (4.2%), primary selection 5,468 (6.8%), second dimmed 203,786 (5.7%), second
selection 7,511 (9.4%). That is at least 14 times the largest baseline, so it isn't noise over time.

What it is and isn't:

- **Edges only.** Flat selection pixels were wrong 5 of 51,785 times on the primary and 29 of 53,300 on the second.
  Pixels with local contrast of 3 or less were wrong 0.003–0.3% of the time; contrast of 16 or more, 48–68%.
- **Not misalignment.** A sub-pixel search from -1.5 to 1.5 px on nine tiles per monitor found the best match at (0, 0)
  on every tile with detail.
- **Not a colour map or sRGB/linear handling.** The value-to-value map has means close to the diagonal, with a spread of
  typically 2–7 levels per value, so the result depends on neighbours rather than on the value. Red and blue aren't
  swapped: comparing against a swapped frame gives several times more wrong pixels.
- **Edges come back sharper than the source.** A linear fit of the recapture against the frame gave negative neighbour
  weights (−0.03 to −0.08). A synthetic pattern of 1 px columns at 64 and 192 came back with values from 0 to 211,
  an overshoot.
- **Not GPUI's image path.** The same 1 px columns drawn as 200 plain quads, with no image or texture sampling, came
  back wrong by the same amount: mean absolute error 38.43 for quads and 38.30 for the image, across the same range.

So the change happens between GPUI's drawing and the GDI readback: in composition, the display driver, or the readback itself. Nothing observed the displayed pixels directly. The GPU is an
AMD RX 7900 XTX, and its driver's registry key has `KMD_USUSharpeningDegree` = 0.8. That suggests a driver sharpening
feature but doesn't show it applies here, and it wasn't toggled because no system setting could be changed in this test.
GPUI's other presentation path was tried too: `GPUI_DISABLE_DIRECT_COMPOSITION=1` changed nothing (two debug-build runs
each; primary dimmed pixels off by more than 3 were 130,452 and 132,332 with it unset, 131,139 and 131,202 with it set,
and the second monitor was identical). No fix was made. A person comparing the frozen image with the live desktop might see
slightly crisper edges, which nobody has checked. A visual comparison can't tell these causes apart, because the frozen
image is itself a readback; only an independent capture can.

### The approach that worked

- **Capture first:** `Monitor::all()` then `capture_image()` one monitor at a time, before any window exists. `xcap`'s
  default capture is GDI `BitBlt`, and it returns an `RgbaImage`. `xcap::image` re-exports `image`, so no direct
  dependency was needed.
- **Image:** copy the frame, swap to BGRA, build `Arc<RenderImage>` from `Frame::new`, and draw with
  `img(...).absolute().size_full().object_fit(ObjectFit::Fill)`. The original RGBA frame is kept for cropping, so each
  monitor's image is held twice in memory, plus a GPU texture.
- **Window:** `WindowKind::PopUp` gives `WS_EX_TOPMOST | WS_EX_TOOLWINDOW`, but with a caption-style frame. Pass
  `WindowBounds::Fullscreen(display.bounds())`, `display_id: Some(display.id())`, `titlebar: None` and `show: true`.
  GPUI then runs its fullscreen path, which strips the frame and sets the monitor's exact physical bounds.
- **Pairing monitors:** `xcap`'s monitor id and GPUI's `DisplayId` are both the `HMONITOR`. `xcap` truncates it to 32 bits,
  so the example compares the low 32 bits.
- **Selection:** stored in physical pixels and drawn at `physical / scale`, so the edges land on device pixels. The dim
  is four rectangles around the selection, so the image inside stays untouched. The crop is
  `imageops::crop_imm(frame, x, y, width, height)`.
- **Closing:** `cx.defer` a loop over `cx.windows()` calling `window.remove_window()`. It has to be deferred because a
  window can't be updated from inside its own update. The default quit mode then ends the app, which the real app would
  replace with `QuitMode::Explicit`. `remove_window()` hides the window only when the message loop next runs, so slow
  work such as encoding must stay off the main thread or the overlay stays frozen on screen until it finishes.

### Limits

- **Don't call `window.activate_window()` lightly.** On Windows, GPUI's implementation sends a synthetic Alt press and
  release through `SendInput` to win the foreground. The spike relies on `focus: true` at creation instead.
- The window is created visible with its frame, then the fullscreen switch runs in a task on the next executor tick.
  Measured: each window first appears with a caption style at slightly oversized bounds, (-8, -4)-(2568, 1444) on the
  primary, and has its final style and bounds about 70 ms later. Thin strips of the screen sampled during that time
  showed no white, black or title bar. The `show: false` route only reveals the window through `activate_window()`.
- Selection is per monitor. A drag stops at its last position on the starting monitor and doesn't continue on the next
  one, and releasing on the other monitor ends it, as observed with real input. From the code, GPUI calls `SetCapture`
  on mouse down, so the release arrives through `on_mouse_up_out`, while `on_mouse_move` needs the pointer over the
  window.
- `WS_EX_TOOLWINDOW` means no taskbar button and, normally, no Alt+Tab entry.
- 157–194 ms passes between finishing the capture and both windows existing. That covers the BGRA conversion and
  window and renderer setup, and wasn't broken down further.

### Hands-on checks

Run 2026-11-08 by an agent on the owner's desktop, with no person watching. Drags, clicks and Esc were synthesized with
`SendInput`, with `TORU_SPIKE_AUTOCLOSE_MS` as a safety net. Esc was sent only when the overlay held the foreground. The
three Esc runs exited with code 0 and empty stderr.

- **Launch sampling:** 43 samples, averaging 27.9 ms apart, worst gap 58 ms, of each overlay's style and bounds plus
  thin strips of the screen. A frame shorter than the sampling gap could be missed.
- **Taskbar:** both overlays are above `Shell_TrayWnd` in z-order, and `WindowFromPoint` returns the overlay at each
  monitor's bottom edge. The taskbar strip's brightness was 0.489 and 0.488 of its pre-overlay value, and it stayed
  there with the pointer on the bottom edge.
- **Drags:** the primary drag gave `401x301 at (600, 300)`, and on the second monitor `401x301 at (792, 440)`, matching
  its (8, -1440) origin. Mid-drag the selection was undimmed (brightness ratio 1.00 and 0.97 inside against 0.49 and
  0.51 outside), and the readout crop read "401 x 301 px" on both.
- **Across monitors:** primary to second gave `1x1001 at (1200, 0)` and second to primary gave `1x281 at (1492, 1140)`,
  stopping at the last position over the starting monitor. Release on the other monitor printed the crop both ways.

### Not verified

- **Mixed display scaling isn't available on this machine**, and neither is any scale other than 1. Fractional scales
  are handled in the code but never ran.
- Whether a person sees any flash. The sampling covers only thin strips, and a flash elsewhere or shorter than its gaps
  would be missed.
- How the overlay actually looks to a person, including whether it looks sharper than the live desktop.
- Whether a captured frame is faithful to the screen. The 1 px columns came back changed through the same GDI path that
  produces the frame, so saved screenshots may carry the same edge change, and the capture-to-capture baseline can't
  detect that. Section 5.1 must check this, for example against Windows Graphics Capture or a known pattern drawn by
  another renderer, on this machine and ideally another GPU.
- Whether Toru gets foreground focus when launched from a global hotkey rather than a terminal.
- Hot-plugging, rotated monitors, HDR, and video or protected content.
- Whether composition, the driver or the GDI readback changes the edges; see "Pixel mismatch".

### Owner's manual checklist

Run from PowerShell, with `%USERPROFILE%\.cargo\bin` on `PATH`. The auto-close is a safety net in case Esc fails; it
closes everything after 30 s, and the overlay starts with a fixed selection that your first drag replaces:

```powershell
$env:TORU_SPIKE_AUTOCLOSE_MS = "30000"; cargo run --release -p toru --example spike_overlay
```

Ticked items passed in the checks above with synthesized input; the unticked ones still need a person.

- [ ] Nothing flashes before the frozen image appears: no title bar, white or black frame, or live desktop. Sampled
  only; see "Hands-on checks".
- [ ] On each monitor, the frozen image lines up with what was on screen and looks as sharp as the real desktop. Lines
  up exactly; sharpness needs a person, since edges read back slightly sharper.
- [x] Each monitor is fully dimmed and covers the taskbar, including when the pointer touches the bottom edge.
- [x] Drag-select on the primary monitor: the inside is undimmed, and the readout follows the drag.
- [x] Drag-select on the second monitor, the same way.
- [x] After each drag, the console prints a size that matches the readout, in physical pixels.
- [x] Drag from one monitor onto the other: the drag stops at the last position on the starting monitor, and releasing
  on the other monitor ends it.
- [x] With the primary monitor focused (click it first), Esc closes both overlays.
- [x] Run again, click the second monitor, and press Esc: both overlays close.
- [x] Run again and press Esc without clicking: both overlays close. The second monitor's overlay had the foreground at
  launch, from a terminal launch.

## 1.3 Text input and gallery scrolling

**Answer:** yes to both. `gpui-kit`'s `Input` is a real editable field: click, typing, Backspace, arrows, Shift-selection,
mouse-drag and double-click selection, Ctrl+A/C/V, Ctrl+Z, Tab, a Windows emoji-panel insertion and Unicode surrogate pairs
all work with synthesized input. A gallery of 200 image cards in a virtual list scrolled with no render-to-render interval
over 16.7 ms and render-to-paint work of 0.9–1.0 ms per frame on average; neither shows what reached the screen. **Real IME composition is unverified**: this machine has only the US keyboard
layout, and no IME was installed. Everything else that needs a person is in the checklist below.

Tested 2026-11-08 on Windows 11 build 26200, Rust 1.98.1, `gpui-pre` 0.3.11, `gpui-kit` 0.9.0, release build, on the
primary monitor (2560x1440 at 240 Hz, scale 1), GPU AMD RX 7900 XTX. Code: `crates/toru/examples/spike_gallery.rs`.

### Dependencies

`gpui-kit` 0.9.0 pins `gpui-pre` and its macro and `sum-tree` crates to exactly `=0.3.11`, so it resolves to the
workspace's GPUI with no conflict and no second copy. The widgets come from its `gpui-component` package, reached as
`gpui_kit::component::...`; only `gpui-kit` is added, as a dev-dependency of `crates/toru`. `image` wasn't needed,
because `xcap` re-exports it with PNG support.

`Cargo.lock` gained 167 packages (710 to 877, counting non-Windows platforms). The crates that build on Windows with dev-dependencies went from
289 to 413, so `gpui-kit` adds 124. Not used by this spike, but pulled in and compiled: `markdown`,
`html5ever`, `notify`, `resvg` and `similar`. The `Input` itself needs `ropey` (text storage), `lsp-types` (positions) and
`rust-i18n` (its menu labels), so those can't be trimmed. `tree-sitter` and `syntect` are off by
default. A full release build of the dependencies took about 2 minutes here. Its GPUI feature flags `x11`, `wayland`,
`font-kit` and `runtime_shaders` don't affect Windows.

### Results

The example has 200 cards in rows of 3, with a day-group header every 30 cards. Each thumbnail starts as a distinct
1440x900 RGBA image, is PNG-encoded in memory, then decoded and shrunk on a background thread, so the path matches
reading a PNG file. Three runs per width, `TORU_SPIKE_THUMB_WIDTH` of 280 (matching the card's width at scale 1, the
default) and 560 (for 200% scale):

| Measure | 280 px thumbnails | 560 px thumbnails |
|---|---|---|
| First frame painted, after `main` | 138–151 ms | 141–149 ms |
| Create 200 `InputState`s | 1.1–1.3 ms | 1.1–1.2 ms |
| Memory those 200 inputs add (private) | 2.3 MB, about 12 KB each | same |
| All thumbnails decoded and ready, after `main` | 308–326 ms | 328–340 ms |
| Decode and downscale, per image | 5.0–6.4 ms | 7.3–8.6 ms |
| Thumbnail memory held for drawing, per image | 191 KB | 765 KB |
| After all 200 uploaded: working set | 109 MB | 237 MB |
| After all 200 uploaded: private bytes | 176 MB | 649 MB |
| Dedicated GPU memory, peak | 88 MB | 433 MB |

The sweep scrolls the whole list twice, 30 px per frame (about 7,200 px/s at 240 Hz), once every thumbnail is decoded,
with `TORU_SPIKE_AUTOSCROLL=1`. The first pass uploads each thumbnail to the GPU on first sight and the second draws
them from the atlas. "Interval" is the time between frames, measured at `render`, and "work" is `render` to the end of
paint. Ranges cover all six runs (three per thumbnail width; the widths didn't differ), roughly 557 frames each:

| Metric | Pass 1 | Pass 2 |
|---|---|---|
| Interval average | 4.26–4.30 ms | 4.19–4.28 ms |
| Interval p99 | 6.9–7.4 ms | 6.9–7.3 ms |
| Interval worst | 7.4–7.8 ms | 7.4–8.2 ms |
| Intervals over 4.17 ms (240 Hz target) | 308–319 | 294–331 |
| Intervals over 6.25 ms (1.5x target) | 33–42 | 24–40 |
| Intervals over 16.7 ms | 0 | 0 |
| Intervals over 33 ms | 0 | 0 |
| Render-to-paint work, average | 0.92–1.04 ms | 0.87–0.95 ms |
| Render-to-paint work, worst | 2.0–2.7 ms | 1.6–2.0 ms |

Intervals over 4.17 ms hover around that value, as expected for a 240 Hz clock with jitter. Between 4% and 8% were over
6.25 ms, but the averages leave room for only about 3–18 truly missed refreshes per pass, so most are probably late
render callbacks that the next, shorter interval made up. Render-to-paint work never came close to these values, and the
intervals only cover `render` calls, so they don't show what reached the screen. Pacing varies between runs: other runs of this sweep, on earlier builds and one on the final code,
averaged 4.6 ms between frames with almost none over 6.25 ms, and still none over 16.7 ms.

Wheel and scrollbar-drag runs (synthesized): render-to-paint work averaged 1.14–1.23 ms with a worst of 2.1 ms, which
leaves out the renderer's own work and `Present`. Intervals in those runs follow the input events, not the display (Windows' timer
limits the script's notches to about 15.6 ms apart). A few worst values of 216–452 ms are script pauses between bursts,
counted because the run is one continuous interval series until 300 ms of quiet; they aren't hitches.

| What was tested | Result | How it was checked |
|---|---|---|
| Click a field focuses it, typing appends | Works | `card 0 focused`, then a value event per key |
| Backspace | Works | Value lost the last character |
| Left/Right arrows, typing mid-string | Works | `hello woxrl` from the expected caret position |
| Shift+arrow selection, copy, replace | Works | Clipboard held the 3 selected characters, typing replaced them, screenshot showed the highlight |
| Ctrl+A, Ctrl+C, Ctrl+V, Ctrl+Z | Works | Clipboard text and values; undo restored the previous value |
| Mouse-drag selection, double-click word | Works | Clipboard held the dragged range; double-click gave `alpha` |
| Tab, Shift+Tab | Works over rendered fields | `focused`/`blurred` events in field order |
| Caret and selection drawing | Works | Screenshots of the window |
| Focus ring | Visible | `ring`-coloured border with a soft halo when focused |
| Unicode via `KEYEVENTF_UNICODE` (CJK, emoji) | Works | `日本語😀`, one value event per character, exact code points |
| Backspace over an emoji | Removes the whole surrogate pair | Code points in the value |
| Windows emoji panel (Win+.), Enter inserted one | Works | Value gained `🫡`; the panel left the example's window in the foreground |
| Real IME composition | **Not tested** | No IME installed; see "Not verified" |
| Wheel scrolling | Works | `manual scroll, now at -3120` |
| Scrollbar thumb drag, top to bottom and back | Works | `manual scroll, now at -16722`, screenshot at the end of the list |

### The approach that worked

- **Setup:** call `gpui_kit::init(cx)` and open the window with `gpui_kit::open_window`. Its `Root` holds the Tab and
  Shift+Tab bindings.
- **List:** `v_virtual_list(view, id, Rc<Vec<Size>>, render_rows)` with a `VirtualListScrollHandle` and
  `.vertical_scrollbar`. Row height is known up front (header or row of three cards). Only the visible rows plus one
  extra below are built each frame. Rows must have `.w_full()`: the list sizes itself from its first row.
- **Inputs:** one `Entity<InputState>` per card, created up front and rendered with `Input::new(&state)`. The element
  only exists while its row is on screen, but the entity keeps the text.
- **Thumbnails:** decode and shrink on the background executor, swap to BGRA, build `Arc<RenderImage>`, and draw with
  `img(...).object_fit(ObjectFit::Cover)`.
- **Field events:** subscribe to each state's `InputEvent` (`Change`, `Focus`, `Blur`, `PressEnter`).

### Inputs: all live or created lazily?

All 200 can hold a live `InputState` from the start. They cost about 12 KB and 6 µs each, and the expensive part
(the element, its layout, focus and caret) only runs for the visible rows. For a history with thousands of cards,
create a day's states when that day loads, the same way thumbnails are loaded.

What that means:

- **Edits are kept** across scrolling, including out and back, because the text lives in the entity. Checked with
  `abc` typed, the card scrolled away and back, then `d` typed giving `abcd`.
- **Focus follows the element.** Scrolling the focused card out of the rendered rows fires `Blur`; scrolling it back fires
  `Focus` again. While it's off-screen, keystrokes go nowhere and nothing is lost or typed elsewhere (`zz` changed
  nothing). Treat `Blur` from scrolling as distinct if a rename commits on blur, so that scrolling doesn't commit.
- **Tab only reaches rendered fields.** It runs through the visible rows and the extra row below, onto cards that are off-screen,
  then wraps to the top row, and it doesn't scroll the list. Shift+Tab wraps the other way. DESIGN.md §7's "Tab through
  controls" needs the app to scroll the focused card into view on keyboard focus changes, with
  `VirtualListScrollHandle::scroll_to_item` and its own handling at the edges. That wasn't built here.

### DESIGN.md §7 support

- **Visible focus:** the default `Input` shows a border in `theme.ring` plus a soft halo. It is a 1 px edge, not §7's 3 px ring
  and halo, so section 3.2 will have to draw its own focus ring or restyle this one (`focus_bordered(false)` and theme
  values). That's a styling job, not a blocker.
- **Tab order:** the state's focus handle is a tab stop, and Tab follows render order, within the limit above.
  `Input::tab_index` stores its value but nothing reads it in 0.9.0, so it can't set the order.
- **`.role()` / `.aria_label()`:** `Input` has both. The role defaults to `TextInput`, and the example sets `aria_label`.
  Whether a screen reader reads them on Windows belongs to section 1.5 and wasn't checked.

### Limits

- **GPU memory grows quickly with thumbnail size.** From the code, GPUI's atlas uses 1024x1024 textures and keeps each image
  whole in one, and the numbers fit about 2 thumbnails to a texture at 560x350: 433 MB of GPU memory for about 150 MB of pixels. At
  280x175 far more fit per texture, and all 200 used 88 MB. Make thumbnails no larger than needed: display width times
  scale factor.
- **The atlas never forgets on its own.** An uploaded thumbnail stays on the GPU until `window.drop_image` is called, so
  a long history needs dropping as days scroll out of view or are collapsed. Not exercised here.
- **Row heights must be known up front** for the list, so day-group Collapse has to rebuild the size list.
- From the code, the list renders row 0 (the header here) once more every frame to measure width, so keep it cheap.

### Hands-on checks

Run 2026-11-08 by an agent on the owner's desktop, with no person watching. All input was synthesized with `SendInput`
into the example's own window, with the foreground and the window under the pointer checked before every send. Window
contents were checked from screenshots of the window alone, saved to the temp folder and deleted. The owner's clipboard
text was saved first and restored afterwards.

- Another app (`LGMonitorAppManager`) took the foreground once during a run. The foreground check stopped the script without
  sending anything further, and the test was re-run; nothing typed reached that app.
- The first run missed one copy: the shift-selection Ctrl+C left the clipboard unchanged. Later runs had 8 correct copies in
  8 tries (selection, drag and double-click). The cause wasn't determined.
- One of the 15 window screenshots came back fully black (mean brightness 0), in the first run; the other 14 weren't blank,
  and the 4 of those viewed by eye showed the expected window. Cause not determined.
- The emoji panel test ran on a build differing only by a tiny layout fix; the example's input path is unchanged. The panel
  stayed open over the window after inserting, and closed when focus left; it was confirmed hidden afterwards.

### Not verified

- **IME composition.** No Japanese, Chinese or other IME is installed here, and none was installed. Code reading suggests it
  should work: GPUI handles `WM_IME_COMPOSITION` and positions the candidate window from the caret, and `InputBaseState`
  implements the marked-text input handler. It stays unverified.
- Right-click context menu in the field. It's a native popup and wasn't opened.
- Touch or pen drag scrolling, and any display scale other than 1. Thumbnails at 2x are measured but only displayed at 1x.
- Frame timing at presentation. Intervals are between `render` calls and work stops at the end of paint, before the renderer's own CPU work and `Present`; GPU present time and what reaches
  the screen weren't measured (no PresentMon here).
- A second, slower GPU or the 60 Hz monitor. The second monitor is 60 Hz and the window ran on the 240 Hz primary.
- Real screenshot PNGs read from disk. These synthetic captures average 85 KB; real ones are usually larger, so decode
  will be slower.
- Dropped-input cases: invalid filename characters and multi-line paste into a name field, which belong to section 5.5.
- Screen reader behaviour (section 1.5).
- How scrolling feels to a person.

### Owner's manual checklist

Run from PowerShell, with `%USERPROFILE%\.cargo\bin` on `PATH`: `cargo run --release -p toru --example spike_gallery`.
Field events and scroll timings print to the console. Ticked items passed in the checks above with synthesized input;
the unticked ones need a person.

- [x] Fields accept typing, editing and selection with the keyboard and mouse.
- [x] Ctrl+A, Ctrl+C, Ctrl+V and Ctrl+Z behave as in any text box.
- [x] Tab moves between fields and the focus ring is visible.
- [x] Wheel scrolling and dragging the scrollbar move through all 200 cards.
- [ ] Scrolling feels smooth by hand, on both the 240 Hz and the 60 Hz monitor.
- [ ] With a Japanese or other IME installed, composing text in a field: the preedit is underlined in the field, the
  candidate window sits at the caret, and committing inserts the text once.
- [ ] Right-click a field: the Cut/Copy/Paste menu appears and works.
