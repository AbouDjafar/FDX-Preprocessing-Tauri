# Repository Instructions

## Project Context

- Read `docs/TRANSMISSION-TAURI.md` for the current compact handover, then
  `docs/TAURI-CONTRACT.md` for interfaces. `docs/TRANSMISSION.md` describes the
  retained Python reference, not the current application architecture.
  Then inspect only the relevant modules and tests rather than rereading the
  whole repository. Keep this handover free of personal paths and private data.

## Build and Delivery

- Use GitHub Actions for ALL dependency installation, compilation, packaging,
  executable testing and release publication. Do not install Node/Rust/Python
  dependencies or run local builds. Inspect the interface through GitHub Pages.
- Commit sources and small required assets only. Never commit executables,
  installers, distribution archives, build directories, or large binary files.
- Publish verified binaries as GitHub Release assets, not in Git history.
- The user downloads releases manually. Do not automatically download, install,
  or launch a release on the user's computer.
- Do not repeat completed builds or processing merely to change the delivery
  approach. Reuse successful verified runs when appropriate.
- Keep personal workbooks, private dictionaries, local projects, and sensitive
  logs local. GitHub Actions tests must use synthetic or explicitly approved
  public data. Heavy tasks involving private data stay local unless the user
  explicitly authorizes their transfer.
- Fast focused checks may run locally; use the workflows in
  `.github/workflows/` for full Windows builds and release delivery.
