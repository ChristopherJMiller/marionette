//! Input simulation via ydotool
//!
//! This module provides cross-platform input simulation by shelling out to ydotool,
//! which uses uinput at the kernel level and works on both X11 and Wayland.

use tokio::process::Command as AsyncCommand;

/// Click at screen coordinates
pub async fn click(x: i32, y: i32, button: &str) -> anyhow::Result<()> {
    // Move mouse to position
    let move_status = AsyncCommand::new("ydotool")
        .args(["mousemove", "--absolute", "-x", &x.to_string(), "-y", &y.to_string()])
        .status()
        .await?;

    if !move_status.success() {
        anyhow::bail!("ydotool mousemove failed");
    }

    // Small delay to ensure move completes
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    // Click
    let button_code = match button {
        "left" => "0xC0",      // Left button click (down + up)
        "right" => "0xC1",     // Right button click
        "middle" => "0xC2",    // Middle button click
        _ => "0xC0",           // Default to left
    };

    let click_status = AsyncCommand::new("ydotool")
        .args(["click", button_code])
        .status()
        .await?;

    if !click_status.success() {
        anyhow::bail!("ydotool click failed");
    }

    Ok(())
}

/// Type text
pub async fn type_text(text: &str, delay_ms: u32) -> anyhow::Result<()> {
    let status = AsyncCommand::new("ydotool")
        .args(["type", "--key-delay", &delay_ms.to_string(), "--", text])
        .status()
        .await?;

    if !status.success() {
        anyhow::bail!("ydotool type failed");
    }

    Ok(())
}

/// Press a key with optional modifiers
pub async fn key_press(key: &str, modifiers: &[String]) -> anyhow::Result<()> {
    // Build the key string with modifiers
    // ydotool key format: key[:state] where state is 1 for down, 0 for up, or omit for press
    // For modifiers, we need to press them down, press the key, then release modifiers

    // Delay before starting key press to ensure system is ready
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Map common key names to ydotool key codes
    let key_code = map_key_to_code(key);

    let mut args: Vec<String> = vec!["key".to_string()];

    // Press modifiers down
    for modifier in modifiers {
        let mod_code = map_modifier_to_code(modifier);
        args.push(format!("{}:1", mod_code)); // Press down
    }

    // Explicitly press down and release the main key with delay between
    args.push(format!("{}:1", key_code)); // Key down
    args.push("50".to_string()); // 50ms delay (ydotool interprets non-keycode values as delays)
    args.push(format!("{}:0", key_code)); // Key up

    // Release modifiers (in reverse order)
    for modifier in modifiers.iter().rev() {
        let mod_code = map_modifier_to_code(modifier);
        args.push(format!("{}:0", mod_code)); // Release
    }

    tracing::debug!("Executing ydotool key with args: {:?}", args);

    let output = AsyncCommand::new("ydotool")
        .args(&args)
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::bail!("ydotool key failed: stderr={}, stdout={}", stderr, stdout);
    }

    Ok(())
}

/// Map human-readable key names to ydotool key codes
fn map_key_to_code(key: &str) -> String {
    // ydotool uses Linux input event codes
    // See: /usr/include/linux/input-event-codes.h
    match key.to_lowercase().as_str() {
        // Special keys
        "return" | "enter" => "28".to_string(),    // KEY_ENTER
        "escape" | "esc" => "1".to_string(),       // KEY_ESC
        "tab" => "15".to_string(),                  // KEY_TAB
        "backspace" => "14".to_string(),            // KEY_BACKSPACE
        "space" => "57".to_string(),                // KEY_SPACE
        "delete" => "111".to_string(),              // KEY_DELETE
        "insert" => "110".to_string(),              // KEY_INSERT
        "home" => "102".to_string(),                // KEY_HOME
        "end" => "107".to_string(),                 // KEY_END
        "pageup" => "104".to_string(),              // KEY_PAGEUP
        "pagedown" => "109".to_string(),            // KEY_PAGEDOWN

        // Arrow keys
        "up" => "103".to_string(),                  // KEY_UP
        "down" => "108".to_string(),                // KEY_DOWN
        "left" => "105".to_string(),                // KEY_LEFT
        "right" => "106".to_string(),               // KEY_RIGHT

        // Function keys
        "f1" => "59".to_string(),
        "f2" => "60".to_string(),
        "f3" => "61".to_string(),
        "f4" => "62".to_string(),
        "f5" => "63".to_string(),
        "f6" => "64".to_string(),
        "f7" => "65".to_string(),
        "f8" => "66".to_string(),
        "f9" => "67".to_string(),
        "f10" => "68".to_string(),
        "f11" => "87".to_string(),
        "f12" => "88".to_string(),

        // Letters (lowercase)
        "a" => "30".to_string(),
        "b" => "48".to_string(),
        "c" => "46".to_string(),
        "d" => "32".to_string(),
        "e" => "18".to_string(),
        "f" => "33".to_string(),
        "g" => "34".to_string(),
        "h" => "35".to_string(),
        "i" => "23".to_string(),
        "j" => "36".to_string(),
        "k" => "37".to_string(),
        "l" => "38".to_string(),
        "m" => "50".to_string(),
        "n" => "49".to_string(),
        "o" => "24".to_string(),
        "p" => "25".to_string(),
        "q" => "16".to_string(),
        "r" => "19".to_string(),
        "s" => "31".to_string(),
        "t" => "20".to_string(),
        "u" => "22".to_string(),
        "v" => "47".to_string(),
        "w" => "17".to_string(),
        "x" => "45".to_string(),
        "y" => "21".to_string(),
        "z" => "44".to_string(),

        // Numbers
        "0" => "11".to_string(),
        "1" => "2".to_string(),
        "2" => "3".to_string(),
        "3" => "4".to_string(),
        "4" => "5".to_string(),
        "5" => "6".to_string(),
        "6" => "7".to_string(),
        "7" => "8".to_string(),
        "8" => "9".to_string(),
        "9" => "10".to_string(),

        // Default: try to parse as raw code
        other => other.to_string(),
    }
}

/// Map modifier names to ydotool key codes
fn map_modifier_to_code(modifier: &str) -> String {
    match modifier.to_lowercase().as_str() {
        "ctrl" | "control" => "29".to_string(),     // KEY_LEFTCTRL
        "alt" => "56".to_string(),                   // KEY_LEFTALT
        "shift" => "42".to_string(),                 // KEY_LEFTSHIFT
        "super" | "meta" | "win" => "125".to_string(), // KEY_LEFTMETA
        other => other.to_string(),
    }
}
