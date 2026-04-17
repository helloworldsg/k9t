# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Development Commands

```bash
cargo build                        # Debug build
cargo build --release              # Release build (~8MB binary)
cargo test --workspace             # Run all tests
cargo test -p k9t-app              # Run tests for a specific crate
cargo fmt --all -- --check         # Check formatting
cargo clippy --workspace -- -D warnings  # Lint
cargo run -- --context my-cluster  # Run with kube context
cargo run -- --namespace default   # Run with specific namespace
cargo run -- --regex-namespace-pods "plt/api-.*"  # Filter pods by regex
```

## Architecture

k9t is a Rust terminal UI for Kubernetes, organized as a Cargo workspace with 4 crates:

- **`k9t-core`** — Kubernetes client (`create_client`, `resolve_context_name`), reflector (`PodReflector` using kube-rs watch streams), pod actions (`delete_pod`, `restart_deployment`, `scale_deployment`, `cordon_node`, `drain_node`), and resource data types (`PodInfo`, `ContainerDetail`, `ContainerPortInfo`, plus `NodeInfo`/`DeploymentInfo`/`ServiceInfo`/`EventInfo` for future resource views). The reflector uses `kube::runtime::reflector` with `WatchStreamExt` to maintain a live store of pods.

- **`k9t-app`** — Application state machine (`App` struct) and all input handling. `Mode` enum drives the UI: `Normal`, `CommandPalette`, `Search`, `Help`, `NamespacePicker`, `ContextPicker`, `ContainerPicker(intent)`, `ContainerActions`, `ConfirmAction`, `SetImageInput`, `PortForwardInput`. Key interactions produce `pending_shell: Option<ShellCommand>` (for suspend/resume kubectl subprocesses) or `pending_async_action: Option<AsyncAction>` (for direct K8s API calls like delete/restart). Config loading (`Config::load`) searches JSON then TOML paths in priority order.

- **`k9t-ui`** — Ratatui rendering: themes (`Theme` struct with 7 built-in themes loaded via `Shift+T`), layout (`AppLayout` splits terminal into header/namespace_bar/table/footer), and widget modules (`resource_table`, `header`, `footer`, `namespace_bar`, `namespace_picker`, `context_picker`, `container_picker`, `container_actions`, `command_palette`, `confirm_dialog`, `toast`).

- **`k9t`** — Binary entry point. The `main()` event loop uses `tokio::select!` over crossterm `EventStream` and a tick interval. On each tick it reads the reflector store. `run_subcommand()` suspends the TUI, runs kubectl (with `less`/`jq`/`bat` pipeline), then resumes. Context switches re-create the kube client and restart the reflector.

### Data flow

1. `PodReflector::start(client)` spawns a background watch stream
2. Main loop calls `reflector.store()` on each tick → `app.set_pods(pods)`
3. User keys → `App::update(AppEvent::Key(...))` → mutates `App` state, possibly sets `pending_shell` or `pending_async_action`
4. Main loop checks `pending_shell`/`pending_async_action` after each event, executes the action
5. Terminal re-renders each frame by reading `App` state directly

### Key patterns

- **ShellCommand** has `fallback_commands` for exec (tries `sh` → `/bin/sh` → `/bin/bash` on exit code 126/127) and `needs_pause` for paged output (describe/yaml/logs piped through `less -RFX`, optionally through `jq` and `bat`)
- **TableRow enum** flattens pods+containers into a single selectable list; `Pod` rows expand to show `Container` sub-rows
- **Namespace selection** uses a "staged" pattern: `staged_namespaces` is modified in the picker, committed on Enter, discarded on Esc
- **Config** supports JSON (`~/.config/k9t.json`) and TOML (`~/.config/k9t/config.toml`) with `CustomCommand` templates using `{{NAMESPACE}}`, `{{POD}}`, `{{CONTAINER}}`, `{{CONTEXT}}`
- **Theme** uses `NO_COLOR=1` env var for monochrome, `COLOR_SCHEME` env var for light mode detection; `Theme::auto()` picks the default

## CI

GitHub Actions runs `cargo check`, `cargo test --workspace`, `cargo fmt --all -- --check`, and `cargo clippy --workspace -- -D warnings` on push/PR to main.