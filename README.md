<img src="assets/icon.svg" width="64" height="64" alt="">

# Toru

A screenshot app for Windows, built with [GPUI](https://www.gpui.rs/). Captures stay temporary until you save them.

## Install

Run `Toru-<version>-setup.exe` (build it with [`installer/build.ps1`](installer/README.md)). It installs for your user
only, with no admin prompt. The installer isn't signed yet, so Windows SmartScreen warns "Windows protected your PC" the
first time: choose **More info**, then **Run anyway**.

## Development

The design lives in [`design/`](design/) and the build plan in [`docs/PLAN.md`](docs/PLAN.md).

## Licence

Toru is under the [MIT licence](LICENSE). The bundled fonts are under the SIL Open Font License 1.1
([`assets/fonts/`](assets/fonts/)).

Created in [T3 Code](https://t3.codes).
