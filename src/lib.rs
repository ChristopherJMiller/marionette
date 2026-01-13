//! Marionette - Window manipulation MCP server for Linux
//!
//! This library provides an MCP (Model Context Protocol) server that enables
//! AI assistants to interact with windows on Linux desktops.
//!
//! ## Features
//!
//! - Window discovery and listing
//! - Window screenshots
//! - Input simulation (click, type, key presses)
//! - Window management (move, resize, focus)
//!
//! ## Supported Environments
//!
//! - X11 (native)
//! - XWayland (games on Wayland sessions)
//! - Native Wayland (wlroots compositors via foreign-toplevel protocol)

pub mod backend;
pub mod core;
pub mod input;
pub mod screenshot;
pub mod server;
