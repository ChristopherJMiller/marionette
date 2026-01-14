# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
# Enter development environment (required for dependencies)
nix develop

# Build debug
cargo build

# Build release
cargo build --release

# Run clippy
cargo clippy

# Format code
cargo fmt

# Test with MCP Inspector (interactive web UI)
npx @modelcontextprotocol/inspector target/debug/marionette

# Basic functionality test
./test_mcp.sh
```

## Architecture

Marionette is an MCP (Model Context Protocol) server that enables AI assistants to manipulate Linux desktop windows. It communicates over stdio using JSON-RPC.

### Module Structure

- **`server.rs`** - MCP server implementation with tool handlers. Each `#[tool(...)]` macro defines an MCP tool exposed to clients. The `MarionetteServer` struct holds the window registry and backend.

- **`core/registry.rs`** - Window registry that assigns stable references (w0, w1, ...) to windows. References persist across `window_list` calls as long as the window exists. Uses `PlatformWindowId` to track windows across X11/Wayland.

- **`backend/`** - Platform abstraction layer. The `WindowBackend` trait defines operations (list, focus, move, resize). Currently only X11 backend is implemented via `x11rb` crate.

- **`input/mod.rs`** - Input simulation via `ydotool`. Maps human-readable key names to Linux input event codes. Includes timing delays to prevent dropped inputs.

- **`screenshot/`** - Window capture via `xcap` crate.

### Key Dependencies

- `rmcp` - MCP protocol implementation with `#[tool]` and `#[tool_router]` macros
- `x11rb` - X11 protocol bindings for window enumeration and management
- `xcap` - Cross-platform screenshot capture
- `ydotool` - Kernel-level input simulation (requires ydotoold service running)

### Important Patterns

The server uses `Arc<RwLock<WindowRegistry>>` for thread-safe window tracking. Tool handlers acquire read locks for lookups and write locks when refreshing the window list.

Input operations shell out to `ydotool` which requires the user to be in the `ydotool` group and the `ydotoold` service to be running.

All logging goes to stderr to keep the stdio MCP channel clean. Use `RUST_LOG=debug` for verbose output.
