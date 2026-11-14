# ADR-0003: Workspace layout and dependency layers

- Status: proposed
- Date: 2026-11-14

## Decision
- **Layout.** Cargo workspace with crates under `crates/`: protocol, core, render, transport,
  printers, mcp, cli, studio, ffi, testkit. Plus `xtask/` and `mobile/{android,ios}`.
- **CLI and studio.** Each is one crate with both lib and bin targets, which keeps the brief's crate
  names. This departs from the prior-art `apps/` split.
- **Layers.**
  - L0: protocol and core, independent of each other.
  - L1: render (uses core) and transport (no workspace dependencies).
  - L2: printers.
  - L3: mcp and ffi.
  - L4: cli and studio.
  - `cargo xtask layers` fails CI on any upward dependency. GUI crates are only allowed at L4, and
    btleplug only in transport.
- **Service traits.** Core defines `Renderer` and `DeviceHub`, and the `Session` receives them.
  This lets `print.*`, `render.preview` and `doc.check` be ordinary core commands without core
  depending on render or printers.
- **Toolchain.** Edition 2024, MSRV 1.95 (set by egui 0.37), pinned stable toolchain.
  `[workspace.dependencies]` holds every version, and `Cargo.lock` is committed.

## Consequences
- Core is testable with fake services and has no I/O.
- Adding a vendor touches only protocol and printers.
