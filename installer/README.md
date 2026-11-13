# Toru installer

An [Inno Setup 6](https://jrsoftware.org/isinfo.php) script that installs Toru for the current user.

## Build

```powershell
winget install --id JRSoftware.InnoSetup -e --scope user   # once
cargo install cargo-about --locked --features cli          # once
powershell -ExecutionPolicy Bypass -File installer\build.ps1
```

`build.ps1` runs `cargo build --release -p toru`, writes `target\THIRD-PARTY-NOTICES.html` with
[cargo-about](https://github.com/EmbarkStudios/cargo-about) (`about.toml` and `about.hbs` at the repo root), then
compiles `toru.iss` with the version from `Cargo.toml`. The installer lands in `installer\output\` (git-ignored).

The notices file is generated on every build rather than committed, so it always matches `Cargo.lock`. If a dependency
update brings in a licence that isn't in `about.toml`'s `accepted` list, cargo-about fails. If it changes a licence
file that `about.toml` pins by checksum, cargo-about only warns, still exits 0 and leaves that notice out, so
`build.ps1` stops on any `[WARN]` or `[ERROR]` line cargo-about prints (a clean run prints none) and deletes the
half-right notices file. Either way: read the new licence, then add it to the list or update the checksum.

## What it does

- Installs to `%LOCALAPPDATA%\Programs\Toru` without admin rights, with a Start menu shortcut and an optional desktop
  shortcut.
- Ships `toru.exe`, Toru's own MIT licence in `licences\LICENSE.txt`, the bundled fonts' licences (SIL Open Font
  License 1.1, plus `assets/fonts/README.md`, which says how the subsets were made) in `licences\fonts\`, and the
  licence notices of the Rust crates compiled into the exe in `licences\THIRD-PARTY-NOTICES.html`. The fonts and everything else the app uses are compiled into the exe.
- Optional "Start Toru when Windows starts": writes the `Toru` value under
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, the same value the app's **Start with Windows** setting uses.
  Toru then starts hidden in the tray (`toru.exe --tray`). Ticking it also clears a "disabled" mark from Windows'
  Startup apps page. The box starts unticked on every install, upgrades included, so an upgrade never turns start-up
  back on; leaving it unticked keeps whatever the app's setting is.
- Setup and the uninstaller ask you to quit Toru first if it is running.
- Uninstalling removes the app, its shortcuts, the Run value and Windows' Startup apps mark for it. It then asks whether to delete Toru's history and
  unsaved captures in `%LOCALAPPDATA%\Toru`, defaulting to **No**; only Toru's own files there are deleted. Screenshots
  you saved are never touched. A silent uninstall keeps the data.

## SmartScreen

The installer and the exe aren't signed, so on first run Windows SmartScreen says "Windows protected your PC". Choose
**More info**, then **Run anyway**.

## Clean-machine test

Run in Windows Sandbox or a fresh VM, so the real install and Run key aren't touched.

1. Copy `installer\output\Toru-<version>-setup.exe` in and run it. Get past SmartScreen (More info → Run anyway). No
   admin prompt appears.
2. Tick "Start Toru when Windows starts" and finish with "Launch Toru" ticked.
3. First run: the window opens in Japandi. Check the headings are in Noto Serif JP and body text in Inter, not Segoe UI.
   Switch the Look to PixelArt (Press Start 2P / VT323) and NeoBrutalism (Archivo Black / Space Grotesk) and check
   those fonts too.
4. `%LOCALAPPDATA%\Programs\Toru\licences\fonts\` holds the six `*-OFL.txt` files and `README.md`;
   `licences\THIRD-PARTY-NOTICES.html` opens in a browser and lists the crates' licences; `licences\LICENSE.txt` is
   Toru's MIT licence.
5. Take a capture, save it, take a second and leave it unsaved.
6. In settings, **Start with Windows** is on. Sign out and back in (or restart): Toru is in the tray and its window is
   hidden. Clicking the tray icon opens the window at its normal size, centred.
7. Turn **Start with Windows** off, sign out and in: Toru doesn't start. Turn it back on.
   Then turn Toru off under Settings → Apps → Startup: Toru's switch shows off after a restart of Toru. Turn Toru's
   switch on: the Startup apps page shows Toru on again.
8. Quit Toru from the tray. Uninstall from Settings → Apps. Answer **No** to deleting history.
9. Check: the saved screenshot is still in `Pictures\Screenshots`; `%LOCALAPPDATA%\Toru` is still there; the Start menu
   shortcut and `%LOCALAPPDATA%\Programs\Toru` are gone; `Toru` is gone from
   `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` (`reg query HKCU\Software\Microsoft\Windows\CurrentVersion\Run`).
10. Install again, uninstall while Toru is running: the uninstaller asks you to close it. Quit it, retry, and answer
    **Yes** to deleting history: `%LOCALAPPDATA%\Toru` is gone and the saved screenshot is still there.
