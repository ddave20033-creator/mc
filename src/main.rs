//! RustCraft: a Minecraft-like game in Rust on wgpu (Vulkan, DirectX 12 or Metal).
//!
//! - `app`: the program around the game: options, key binds, translations, the start-up
//!   splash, F3 statistics and the command-line developer tools.
//! - `client`: the game as played: the window, menus, the player, what they see and do.
//! - `sim`: the simulation, the same for the server and the players: the clock, the world's
//!   rules, felling, grenades, and the server that runs a world (`sim::server`).
//! - `net`: the LAN protocol and connections.
//! - `content`: every block, item and mob, a line each.
//! - `world`: blocks, chunks, terrain generation, fluids, meshing, and the world on disk.
//! - `item`, `entity`: items in use (inventory, crafting, mining, guns); things in the world
//!   that are not blocks (dropped items, block entities, mobs, the player's body).
//! - `model`: geometry built on the CPU (entities, held items, the hand, particles), and the
//!   Blockbench models' data.
//! - `textures`: the block, item and entity textures, from the procedural ones and the
//!   resource packs.
//! - `engine`, `render`: the GPU (wgpu) and the frame's drawing.
//! - `ui`, `audio`: the immediate-mode UI and the menu screens; the sounds.
//! - `util`: small helpers shared by all of it.

// No console window next to the game in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;
mod client;
mod content;
mod engine;
mod entity;
mod item;
mod model;
mod net;
mod render;
mod sim;
mod textures;
mod ui;
mod util;
mod world;

use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, DeviceId, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

#[derive(Default)]
struct App {
    game: Option<client::Game>,
    /// The start-up splash (the logo filling up), while the game gets ready.
    splash: Option<app::splash::Splash>,
    bench: bool,
    /// `--aa-shots <folder>`: anti-aliasing comparison pictures.
    shots: Option<std::path::PathBuf>,
    /// `--test <script> [folder]`: a test script (see `client::testbed`).
    test: Option<(String, Option<std::path::PathBuf>)>,
}

impl App {
    fn shutdown(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(g) = &mut self.game {
            g.on_exit();
        }
        self.game = None;
        event_loop.exit();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.game.is_some() {
            return;
        }
        // 16:9 window at most 1280x720 and at most ~75% of the monitor height (leaves room for the taskbar).
        let mut splash = app::splash::Splash::new();
        splash.set_progress(0.02);
        let mut size = LogicalSize::new(1280.0, 720.0);
        if let Some(m) = event_loop.primary_monitor() {
            let logical = m.size().to_logical::<f64>(m.scale_factor());
            let h = (logical.height * 0.75).min(720.0);
            size = LogicalSize::new(h * 16.0 / 9.0, h);
        }
        let attrs = Window::default_attributes()
            .with_title("Your Worlds")
            .with_inner_size(size)
            .with_min_inner_size(LogicalSize::new(480.0, 320.0))
            // (shown once the start-up screen is drawn, not blank white before it)
            .with_visible(false);
        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("failed to create window"),
        );
        let mut game = client::Game::new(window, self.bench, self.shots.clone(), self.test.clone());
        splash.set_progress(0.1);
        game.frame();
        self.game = Some(game);
        self.splash = Some(splash);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => self.shutdown(event_loop),
            WindowEvent::RedrawRequested => {
                let quit = match self.game.as_mut() {
                    Some(g) => {
                        g.frame();
                        g.quit
                    }
                    None => false,
                };
                if quit {
                    self.shutdown(event_loop);
                }
            }
            other => {
                if let Some(g) = self.game.as_mut() {
                    g.window_event(&other);
                }
            }
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if let (Some(g), DeviceEvent::MouseMotion { delta }) = (self.game.as_mut(), event) {
            g.mouse_motion(delta.0 as f32, delta.1 as f32);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(g) = self.game.as_mut() else {
            return;
        };
        if let Some(splash) = self.splash.as_mut() {
            // Starting up: the window is hidden (and not asked to redraw), so its frames are
            // run from here; the splash shows how far it has got, and when it is full the
            // window takes its place.
            g.frame();
            if g.quit {
                self.shutdown(event_loop);
                return;
            }
            splash.set_progress(g.boot_progress().unwrap_or(1.0));
            if splash.full() && g.boot_progress().is_none() {
                self.splash = None;
                g.show_window();
            }
            return;
        }
        // With a frame limit: wait for the next frame's time taking in input meanwhile; the
        // last moment `frame` spins itself, to start it on time.
        let early = std::time::Duration::from_micros(1500);
        match g.next_frame_due() {
            Some(t) if t > std::time::Instant::now() + early => {
                event_loop.set_control_flow(ControlFlow::WaitUntil(t - early));
            }
            _ => {
                event_loop.set_control_flow(ControlFlow::Poll);
                g.request_redraw();
            }
        }
    }
}

/// The game has no console window, so a crash would just close it: instead the error and
/// where it happened go into `crash.txt` next to the game and show in a message box.
fn report_crashes() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default(info);
        let place = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "ismeretlen hiba".into());
        let text = format!("A Your Worlds hibával leállt.\n\n{msg}\n\nHely: {place}");
        let _ = std::fs::write("crash.txt", &text);
        message_box(&format!("{text}\n\n(Elmentve: crash.txt)"));
    }));
}

#[cfg(windows)]
fn message_box(text: &str) {
    #[link(name = "user32")]
    extern "system" {
        fn MessageBoxW(hwnd: isize, text: *const u16, caption: *const u16, flags: u32) -> i32;
    }
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let (text, caption) = (wide(text), wide("Your Worlds - hiba"));
    const MB_ICONERROR: u32 = 0x10;
    // SAFETY: both strings are NUL-terminated UTF-16 and outlive the call.
    unsafe {
        MessageBoxW(0, text.as_ptr(), caption.as_ptr(), MB_ICONERROR);
    }
}

#[cfg(not(windows))]
fn message_box(text: &str) {
    eprintln!("{text}");
}

fn main() {
    // Saves, settings, skins and resource packs are next to the game, wherever it is
    // started from (a shortcut, another folder). `cargo run` keeps the project folder.
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Into::into)) {
        let dir: std::path::PathBuf = dir;
        if !dir.ends_with("release") && !dir.ends_with("debug") {
            let _ = std::env::set_current_dir(dir);
        }
    }
    report_crashes();
    let args: Vec<String> = std::env::args().collect();
    if app::devtools::run(&args) {
        return;
    }
    let event_loop = EventLoop::new().expect("failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        bench: args.get(1).map(String::as_str) == Some("--bench"),
        shots: (args.get(1).map(String::as_str) == Some("--aa-shots"))
            .then(|| args.get(2).map(Into::into).unwrap_or_else(|| "aa-shots".into())),
        test: (args.get(1).map(String::as_str) == Some("--test"))
            .then(|| (args.get(2).cloned().unwrap_or_else(|| "checks".into()), args.get(3).map(Into::into))),
        game: None,
        splash: None,
    };
    event_loop.run_app(&mut app).expect("event loop error");
}
