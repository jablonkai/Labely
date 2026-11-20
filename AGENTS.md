# AGENTS.md

How coding agents build and test Labely, and how AI agents drive it.

## Build and test

```sh
cargo xtask ci           # fmt, layers, i18n-check, clippy (default and all features), tests
cargo test -p labely-core          # the command engine
cargo test -p labely-mcp           # MCP end to end (stdio and HTTP)
cargo test -p labely-studio        # egui_kittest UI tests
cargo test -p labely-ffi           # FFI engine and native BLE path, on the simulator
cargo xtask bindings kotlin        # generate the UniFFI Kotlin sources into target/bindings
(cd mobile/android && ./gradlew :app:assembleDebug)   # Android app; see mobile/android/README.md
cargo xtask xcframework            # iOS XCFramework and Swift bindings; see mobile/ios/README.md
(cd mobile/ios/LabelyFFI && swift test)              # Swift engine smoke tests (macOS slice)
(cd mobile/ios/LabelyKit && swift test)              # iOS shell logic on the engine (macOS)
cargo xtask package                # studio packages with cargo-packager (ADR-0038): app, dmg, MSI, AppImage, deb
dist build --artifacts=local --target <triple>   # CLI archive with dist
cargo xtask bless                  # rewrite render goldens after an intended change
cargo xtask schema-hash            # print the command-schema hash
```

Rules:
- **Toolchain.** The toolchain is pinned in `rust-toolchain.toml`.
- **Lints.** Library code denies `unwrap`, `expect` and `panic`.
- **Layers.** Dependencies follow `cargo xtask layers` (ADR-0003). GUI crates may only be used
  by `labely-studio`, `rmcp` only by `labely-mcp`, and `uniffi` only by `labely-ffi`.
- **Commands.** Every document change is a command in `crates/core/src/commands/`,
  registered in `registry.rs`.
- **Schema changes.** A command schema change fails `api_version_matches_schema_hash`. When it
  does, bump `API_VERSION` in `crates/core/src/lib.rs` (once per release) and paste the new
  hash into `crates/core/tests/it/schema.rs`. Then refresh the insta snapshot with
  `INSTA_UPDATE=always cargo test -p labely-core`.
- **UI text.** Studio and CLI text goes in `crates/i18n/locales/{en,hu}/*.ftl`. The two
  languages must have the same keys.
- **File access.** A new command that reads or writes files must call `Ctx::check_path`
  (ADR-0012).

## Drive Labely as an agent

Labely exposes the same command registry to every frontend. Pick one of these routes.

| Route | Use it when |
|---|---|
| MCP: `labely mcp` (stdio) | Use it from an MCP client such as Claude Code, Claude Desktop or Cursor. If the studio is running, it bridges to the studio, so the user sees every change live. Otherwise it runs headless. |
| CLI: `labely run` | Use it for one-off commands and scripts in a shell. |

MCP client config:

```json
{ "mcpServers": { "labely": { "command": "labely", "args": ["mcp"] } } }
```

- **Headless flags.** `--root DIR` (repeatable) is the directory the agent may read and write;
  the default is the current directory. `--allow-print` lets the agent print. `--headless`
  never bridges, and `--connect` only bridges.
- **Studio.** The studio serves MCP on `127.0.0.1:7878/mcp`. It requires the bearer token from
  `<app-data>/mcp-connection.json` and refuses non-local `Origin` headers. Agents can print
  only while "Allow agents to print" is ticked.

Read next:
- [docs/agents/usage.md](docs/agents/usage.md): the workflow, envelopes and errors.
- [docs/agents/tools.md](docs/agents/tools.md): the MCP tools and the command catalogue.
- [docs/agents/batch-examples.md](docs/agents/batch-examples.md): ready-to-use batches.
