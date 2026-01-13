//! Screenshot capture using xcap
//!
//! This module provides cross-platform screenshot capabilities using the xcap crate,
//! which handles both X11 and Wayland (via portal) transparently.

use crate::core::registry::PlatformWindowId;
use image::ImageEncoder;

/// Capture a screenshot of a specific window
pub async fn capture_window(platform_id: &PlatformWindowId) -> anyhow::Result<Vec<u8>> {
    // xcap is not async, so we run it in a blocking task
    let platform_id = platform_id.clone();

    let result = tokio::task::spawn_blocking(move || {
        capture_window_blocking(&platform_id)
    }).await??;

    Ok(result)
}

fn capture_window_blocking(platform_id: &PlatformWindowId) -> anyhow::Result<Vec<u8>> {
    let PlatformWindowId::X11(window_id) = platform_id else {
        anyhow::bail!("Only X11 windows are currently supported for screenshots");
    };

    // Get all windows and find the one with matching ID
    let windows = xcap::Window::all()?;

    let window = windows
        .into_iter()
        .find(|w| w.id().ok() == Some(*window_id))
        .ok_or_else(|| anyhow::anyhow!("Window not found for screenshot"))?;

    // Capture the window
    let image = window.capture_image()?;

    // Encode as PNG
    let mut buffer = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut buffer);
    encoder.write_image(
        image.as_raw(),
        image.width(),
        image.height(),
        image::ExtendedColorType::Rgba8,
    )?;

    Ok(buffer)
}

/// Capture a region of the screen
#[allow(dead_code)]
pub async fn capture_region(x: i32, y: i32, width: u32, height: u32) -> anyhow::Result<Vec<u8>> {
    let result = tokio::task::spawn_blocking(move || {
        capture_region_blocking(x, y, width, height)
    }).await??;

    Ok(result)
}

fn capture_region_blocking(x: i32, y: i32, width: u32, height: u32) -> anyhow::Result<Vec<u8>> {
    // Capture the primary monitor
    let monitors = xcap::Monitor::all()?;
    let monitor = monitors
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No monitors found"))?;

    let full_image = monitor.capture_image()?;

    // Crop to region
    let cropped = image::imageops::crop_imm(
        &full_image,
        x.max(0) as u32,
        y.max(0) as u32,
        width,
        height,
    ).to_image();

    // Encode as PNG
    let mut buffer = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut buffer);
    encoder.write_image(
        cropped.as_raw(),
        cropped.width(),
        cropped.height(),
        image::ExtendedColorType::Rgba8,
    )?;

    Ok(buffer)
}
