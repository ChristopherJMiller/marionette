# Marionette

A Model Context Protocol (MCP) server for Linux desktop window manipulation. Marionette allows AI assistants to interact with your desktop windows - perfect for game automation, UI testing, and interactive desktop applications.

## Features

- **Window Discovery**: List and filter windows by title or class
- **Screenshots**: Capture window contents for visual inspection
- **Input Simulation**: Type text and press keys via ydotool
- **Window Management**: Focus, move, and resize windows
- **Stable References**: Windows are assigned stable refs (w0, w1, w2...) that persist across calls
- **X11 & Wayland Support**: Works on both X11 and XWayland applications

## Architecture

Marionette uses:
- **X11 backend** (via `x11rb`) for window enumeration and management
- **xcap** for cross-platform screenshots
- **ydotool** for kernel-level input simulation that works on both X11 and Wayland
- **rmcp** for MCP protocol implementation over stdio

## Installation

### Using Nix Flakes (Recommended)

The easiest way to use marionette is directly from GitHub:

```json
{
  "mcpServers": {
    "marionette": {
      "command": "nix",
      "args": ["--quiet", "run", "github:ChristopherJMiller/marionette"]
    }
  }
}
```

### Building from Source

```bash
# Clone the repository
git clone https://github.com/ChristopherJMiller/marionette.git
cd marionette

# Build with Nix
nix build

# Or build with cargo in a nix shell
nix develop
cargo build --release
```

## NixOS Setup

Marionette requires `ydotool` for input simulation. On NixOS, add this to your configuration:

```nix
{
  # Enable ydotool system service
  programs.ydotool.enable = true;

  # Add your user to the ydotool group
  users.users.yourUsername = {
    extraGroups = [ "ydotool" ];
  };
}
```

After adding this:
1. Run `sudo nixos-rebuild switch`
2. Log out and log back in (or reboot)
3. Verify with: `groups | grep ydotool`

The ydotool system service should be running automatically. Check with: `systemctl status ydotoold.service`

## Usage

### As an MCP Server

Add marionette to your MCP client configuration (e.g., Claude Desktop's `.mcp.json`):

```json
{
  "mcpServers": {
    "marionette": {
      "command": "nix",
      "args": ["--quiet", "run", "github:ChristopherJMiller/marionette"]
    }
  }
}
```

Or use a local build:

```json
{
  "mcpServers": {
    "marionette": {
      "command": "/path/to/marionette/target/release/marionette",
      "args": []
    }
  }
}
```

### Testing with the MCP Inspector

```bash
nix develop
npx @modelcontextprotocol/inspector target/debug/marionette
```

This will open a web interface to test all the tools interactively.

### Testing with Shell Script

A test script is included for basic functionality testing:

```bash
nix develop
./test_mcp.sh
```

## Available Tools

### window_list
List all windows with their references and metadata.

**Parameters:**
- `title_filter` (optional): Filter by window title (substring match)
- `class_filter` (optional): Filter by window class/app name

**Returns:** Array of windows with refs (w0, w1, w2...), titles, classes, geometry, and focus state.

### window_screenshot
Capture a screenshot of a specific window.

**Parameters:**
- `ref` (required): Window reference from window_list (e.g., "w0")
- `format` (optional): "base64" (default) or "file"

**Returns:** Base64-encoded PNG image or file path.

### window_snapshot
Get detailed metadata about a window's current state.

**Parameters:**
- `ref` (required): Window reference (e.g., "w0")

**Returns:** Window title, class, geometry, focus state, visibility, and platform ID.

### window_type
Type text into the currently focused window.

**Parameters:**
- `text` (required): The text to type
- `delay_ms` (optional): Delay between keystrokes (default: 12ms)

### window_key
Press a key or key combination.

**Parameters:**
- `key` (required): Key name (e.g., "Return", "Escape", "a", "F1")
- `modifiers` (optional): Array of modifiers: "ctrl", "alt", "shift", "super"

**Example:** Press Ctrl+C: `{"key": "c", "modifiers": ["ctrl"]}`

### window_click
Click at coordinates within a window.

**Parameters:**
- `ref` (required): Window reference
- `x`, `y` (required): Coordinates within the window
- `button` (optional): "left" (default), "right", or "middle"
- `description` (optional): Human-readable description of what's being clicked

### window_focus
Focus/activate a window, bringing it to the foreground.

**Parameters:**
- `ref` (required): Window reference

### window_move
Move a window to a new position.

**Parameters:**
- `ref` (required): Window reference
- `x`, `y` (required): New position in screen coordinates

### window_resize
Resize a window.

**Parameters:**
- `ref` (required): Window reference
- `width`, `height` (required): New dimensions in pixels

## Example Workflow

```
1. List windows:
   → window_list()
   ← Returns: [{"ref": "w0", "title": "Terminal", ...}, ...]

2. Take a screenshot:
   → window_screenshot(ref: "w0")
   ← Returns: base64 PNG image

3. Type in the window:
   → window_type(text: "echo hello")

4. Press Enter:
   → window_key(key: "Return")

5. Take another screenshot to verify:
   → window_screenshot(ref: "w0")
```

## Technical Details

### Window Registry
Marionette maintains a stable window registry that assigns references (w0, w1, w2...) to windows based on their platform IDs. These references persist across tool calls within the same session.

### Input Timing
The input system includes carefully tuned delays:
- 100ms delay before key press operations (ensures system readiness)
- 50ms delay between key down and key up events (prevents missed keypresses)
- 10ms delay between mouse move and click (ensures position accuracy)
- Configurable typing delay (default 12ms per keystroke)

These delays prevent the common issue of input events being dropped or not registering properly.

### Logging
All logging goes to stderr to keep the stdio MCP channel clean. Set `RUST_LOG=debug` for detailed debugging output.

## Requirements

- **Linux** with X11 or Wayland (XWayland for games)
- **ydotool** system service running (handled automatically on NixOS with `programs.ydotool.enable = true`)
- User must be in the `ydotool` group for input simulation

## Use Cases

- **Game Automation**: Control game windows with AI assistants
- **UI Testing**: Automated testing of desktop applications
- **Desktop Automation**: Automate repetitive desktop tasks
- **Accessibility**: Build custom accessibility tools with AI guidance
- **Screen Recording Bots**: Create bots that can see and interact with applications

## License

MIT

## Contributing

Contributions welcome! This project is in early development and feedback is appreciated.

## Troubleshooting

### Input not working
- Verify ydotool service is running: `systemctl status ydotoold.service`
- Check you're in the ydotool group: `groups | grep ydotool`
- Log out and back in after adding yourself to the group

### Window not found errors
- Run `window_list` first to get current window references
- Window references change between server restarts
- Check that windows are XWayland windows (not native Wayland)

### Screenshots not capturing
- Ensure the window is visible and not minimized
- Check that xcap has necessary permissions
- For Wayland, some compositors may require additional permissions
