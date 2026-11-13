<img src="assets/icon.svg" width="64" height="64" alt="">

# Toru

A screenshot app for Windows, built with [GPUI](https://www.gpui.rs/). Captures stay temporary until you save them.

## Install

Download the latest `Toru-<version>-setup.exe` from the [Releases page](https://github.com/ChacaPatrick/toru/releases/latest)
and run it. It installs for your user only, with no admin prompt. The installer isn't signed yet, so Windows SmartScreen
warns "Windows protected your PC" the first time: choose **More info**, then **Run anyway**.

You only do this once: Toru updates itself after that. To build the installer yourself, see
[`installer/README.md`](installer/README.md).

## Updates

Toru looks for a new release when it starts and every few hours after, downloads it in the background, and then offers
it in its window with the patch notes: **Update** installs it and starts Toru again. **Check for updates** in the
settings column turns the checks off, and **Check now** looks straight away.

## Development

The design lives in [`design/`](design/) and the build plan in [`docs/PLAN.md`](docs/PLAN.md).

Only release builds look for updates. Set `TORU_UPDATE_URL` to the URL of another `toru-update.txt`, which may be a
`file://` one, to try the update flow in any build; `TORU_DATA_DIR` keeps such a run's data out of your own.

## Licence

Toru is under the [MIT licence](LICENSE). The bundled fonts are under the SIL Open Font License 1.1
([`assets/fonts/`](assets/fonts/)).

Created in [T3 Code](https://t3.codes).
