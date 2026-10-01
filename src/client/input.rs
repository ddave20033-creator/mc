//! The keyboard and the mouse: what is held and pressed this frame (`Input`), the keys
//! going to the screen open (a menu, the chat, a text field), the game's keys, and the
//! mouse grabbed while playing.

use crate::client::{Container, Game, Screen};
use crate::app::keys::{Bind, HOTBAR};
use crate::ui::chat::ChatInput;
use glam::Vec2;
use std::collections::HashSet;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Fullscreen};

/// The keyboard and the mouse as they are this frame (`end_frame` lets go of what was only
/// this frame's).
pub(super) struct Input {
    /// The keys held.
    pub(super) keys: HashSet<KeyCode>,
    pub(super) left_down: bool,
    pub(super) right_down: bool,
    /// Pressed this frame.
    pub(super) left_pressed: bool,
    pub(super) right_pressed: bool,
    pub(super) middle_pressed: bool,
    /// The mouse's movement while grabbed, gathered for this frame, and last frame's (the
    /// hand sways with it).
    pub(super) mouse_delta: Vec2,
    pub(super) look_delta: Vec2,
    /// The wheel's notches this frame.
    pub(super) scroll: f32,
    pub(super) cursor_grabbed: bool,
    /// Text typed and backspaces this frame (for the menus' text fields).
    pub(super) typed: String,
    pub(super) backspace: u32,
    /// A hotbar key pressed this frame at an item screen (it swaps with the hotbar).
    pub(super) digit: Option<usize>,
    /// When jump and forward were last pressed (double taps), -1 after one.
    last_space: f32,
    last_w: f32,
    /// The game window is in front (not tabbed out): the others see "away" otherwise.
    pub(super) focused: bool,
    /// Double-tapped forward: sprinting while it is held.
    pub(super) w_sprint: bool,
}

impl Input {
    pub(super) fn new() -> Self {
        Self {
            keys: HashSet::new(),
            left_down: false,
            right_down: false,
            left_pressed: false,
            right_pressed: false,
            middle_pressed: false,
            mouse_delta: Vec2::ZERO,
            look_delta: Vec2::ZERO,
            scroll: 0.0,
            cursor_grabbed: false,
            typed: String::new(),
            backspace: 0,
            digit: None,
            last_space: -1.0,
            last_w: -1.0,
            focused: true,
            w_sprint: false,
        }
    }

    /// Either shift key is held.
    pub(super) fn shift(&self) -> bool {
        self.keys.contains(&KeyCode::ShiftLeft) || self.keys.contains(&KeyCode::ShiftRight)
    }

    /// Either control key is held.
    fn ctrl(&self) -> bool {
        self.keys.contains(&KeyCode::ControlLeft) || self.keys.contains(&KeyCode::ControlRight)
    }

    /// Forward pressed at `now`: a double tap starts sprinting.
    fn tap_forward(&mut self, now: f32) {
        if now - self.last_w < 0.3 {
            self.w_sprint = true;
            self.last_w = -1.0;
        } else {
            self.last_w = now;
        }
    }

    /// Jump pressed at `now`: whether it was a double tap.
    fn tap_jump(&mut self, now: f32) -> bool {
        if now - self.last_space < 0.3 {
            self.last_space = -1.0;
            true
        } else {
            self.last_space = now;
            false
        }
    }

    /// Nothing held any more (the window lost the focus, or the mouse was let go).
    fn release(&mut self) {
        self.w_sprint = false;
        self.last_w = -1.0;
        self.left_down = false;
        self.right_down = false;
    }

    /// The frame is over: what was pressed, moved, scrolled and typed in it is used up.
    pub(super) fn end_frame(&mut self) {
        self.left_pressed = false;
        self.right_pressed = false;
        self.middle_pressed = false;
        self.look_delta = self.mouse_delta;
        self.mouse_delta = Vec2::ZERO;
        self.scroll = 0.0;
        self.typed.clear();
        self.backspace = 0;
        self.digit = None;
    }
}

impl Game {
    /// Holding the sneak key (it also flies down and places chests unjoined).
    pub(super) fn sneaking(&self) -> bool {
        self.bind_down(Bind::Sneak)
    }

    /// The key of this action is held.
    pub(super) fn bind_down(&self, b: Bind) -> bool {
        self.input.keys.contains(&self.settings.keys.get(b))
    }

    pub fn window_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::Resized(size) => self.gfx.gpu.resize(size.width, size.height),
            WindowEvent::CursorMoved { position, .. } => {
                self.ui.set_mouse(Vec2::new(position.x as f32, position.y as f32));
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let pressed = *state == ElementState::Pressed;
                match button {
                    MouseButton::Left => {
                        self.input.left_down = pressed;
                        self.input.left_pressed |= pressed;
                    }
                    MouseButton::Right => {
                        self.input.right_down = pressed;
                        self.input.right_pressed |= pressed;
                    }
                    MouseButton::Middle => self.input.middle_pressed |= pressed,
                    _ => {}
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.input.scroll += match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return;
                };
                if event.state == ElementState::Pressed {
                    // Options: the next key goes to the action waiting for one.
                    if let (Screen::KeyBinds { .. }, Some(i)) =
                        (self.screen, self.menus.options.listening)
                    {
                        if code == KeyCode::Escape {
                            self.menus.options.listening = None;
                        } else if crate::app::keys::bindable(code) {
                            self.settings.keys.0[i] = code;
                            self.menus.options.listening = None;
                        }
                        return;
                    }
                    if self.screen == Screen::Chat {
                        match self.chat.key(code, event.text.as_ref().map(|t| t.as_str())) {
                            ChatInput::None => {}
                            ChatInput::Close => self.resume(),
                            ChatInput::Submit(line) => {
                                self.resume();
                                self.run_command(&line);
                            }
                        }
                        return;
                    }
                    // A menu is moved through with the keyboard too (see `Ui::nav_key`).
                    let menu = !matches!(
                        self.screen,
                        Screen::Playing | Screen::Chat | Screen::Container(_) | Screen::Spectate | Screen::Loading
                    );
                    let shift = self.input.shift();
                    if menu && self.ui.nav_key(code, shift) {
                        return;
                    }
                    if matches!(self.screen, Screen::Container(_)) && self.inv_ui.jei.focused {
                        self.jei_key(code, event.text.as_ref().map(|t| t.as_str()));
                        return;
                    }
                    if self.screen == Screen::Container(Container::Creative) && self.inv_ui.search_focused
                    {
                        self.search_key(code, event.text.as_ref().map(|t| t.as_str()));
                        return;
                    }
                    if self.screen == Screen::Multiplayer {
                        match code {
                            KeyCode::Escape => {
                                self.menus.finder = None;
                                self.settings.save();
                                self.screen = Screen::MainMenu;
                            }
                            KeyCode::Backspace => self.input.backspace += 1,
                            KeyCode::Enter | KeyCode::NumpadEnter => {
                                if !self.menus.mp_address.trim().is_empty() {
                                    let addr = self.menus.mp_address.clone();
                                    self.join_server(&addr);
                                }
                            }
                            _ => {
                                if let Some(t) = &event.text {
                                    self.input.typed.push_str(t);
                                }
                            }
                        }
                        return;
                    }
                    if self.screen == Screen::CreateWorld {
                        match code {
                            KeyCode::Escape => self.screen = Screen::SelectWorld,
                            KeyCode::Backspace => self.input.backspace += 1,
                            KeyCode::Enter | KeyCode::NumpadEnter => self.create_world(),
                            _ => {
                                if let Some(t) = &event.text {
                                    self.input.typed.push_str(t);
                                }
                            }
                        }
                        return;
                    }
                    if !event.repeat {
                        self.key_pressed(code);
                    }
                    self.input.keys.insert(code);
                } else {
                    self.input.keys.remove(&code);
                    if self.settings.keys.is(Bind::Forward, code) {
                        self.input.w_sprint = false;
                    }
                }
            }
            WindowEvent::Focused(true) => self.input.focused = true,
            WindowEvent::Focused(false) => {
                self.input.focused = false;
                self.input.keys.clear();
                self.input.release();
                // Bench and shot runs keep going in the background.
                if self.screen == Screen::Playing && !self.test.any() {
                    self.pause();
                }
            }
            _ => {}
        }
    }

    pub fn mouse_motion(&mut self, dx: f32, dy: f32) {
        if self.input.cursor_grabbed && self.screen == Screen::Playing {
            self.input.mouse_delta += Vec2::new(dx, dy);
        }
    }

    pub(super) fn key_pressed(&mut self, code: KeyCode) {
        let is = |b: Bind| self.settings.keys.is(b, code);
        let digit = HOTBAR.iter().position(|&b| is(b));
        let forward = is(Bind::Forward);
        let stop_sprint = is(Bind::Back) || is(Bind::Sneak);
        let chat = is(Bind::Chat);
        let command = is(Bind::Command) || code == KeyCode::NumpadDivide;
        let drop = is(Bind::Drop);
        let fly = is(Bind::Fly);
        let jump = is(Bind::Jump);
        let fullscreen = is(Bind::Fullscreen);
        let hide_hud = is(Bind::HideHud);
        let debug = is(Bind::Debug);
        let perspective = is(Bind::Perspective);
        let inventory = is(Bind::Inventory);
        let reload = is(Bind::Reload);
        let inspect = is(Bind::Inspect);
        if is(Bind::GunLight) && self.screen == Screen::Playing {
            self.toggle_gun_light();
        }
        if code == KeyCode::Escape {
            match self.screen {
                Screen::Playing => self.pause(),
                Screen::Paused | Screen::Spectate => self.resume(),
                Screen::Container(_) => self.close_container(),
                Screen::SelectWorld => self.screen = Screen::MainMenu,
                Screen::DeleteWorld => self.screen = Screen::SelectWorld,
                Screen::MainMenu | Screen::Dead => {}
                Screen::Connecting => self.cancel_connecting(),
                Screen::Disconnected => self.screen = Screen::MainMenu,
                _ => self.go_back(),
            }
        }
        if fullscreen {
            self.toggle_fullscreen();
        }
        if hide_hud {
            self.hud.hide = !self.hud.hide;
        }
        if debug {
            self.hud.debug = !self.hud.debug;
        }
        if perspective {
            self.me.look.camera.cycle();
        }
        if inventory {
            match self.screen {
                // Spectators have no inventory: the key lists the players to watch.
                Screen::Playing if self.spectator() => self.open_spectate_menu(),
                Screen::Spectate => self.resume(),
                Screen::Playing => self.open_container(if self.creative() {
                    Container::Creative
                } else {
                    Container::Inventory
                }),
                Screen::Container(_) => self.close_container(),
                _ => {}
            }
        }
        if let Screen::Container(_) = self.screen {
            self.input.digit = digit;
            return;
        }
        if self.screen == Screen::Spectate {
            if let Some(i) = digit {
                self.spectate_nth(i);
            }
            return;
        }
        if self.screen != Screen::Playing {
            return;
        }
        if self.spectator() {
            // Nothing in the hands: the hotbar, dropping and reloading do nothing.
            if forward && !self.input.keys.contains(&code) {
                self.input.tap_forward(self.clock.time);
            }
            if stop_sprint {
                self.input.w_sprint = false;
            }
            if chat {
                self.open_chat("");
            } else if command {
                self.open_chat("/");
            }
            return;
        }
        self.book_key(code);
        // Reading the book, the number keys open its chapters.
        let digit = digit.filter(|&i| !self.book_digit(i));
        if let Some(i) = digit {
            self.me.items.hotbar_slot = i;
            self.hud.slot_name_timer = 2.0;
        }
        if reload {
            self.tools.guns.reload_pressed = true;
        }
        // Holding a gun, the inspect key (which may be the fly key) looks it over.
        let inspecting = inspect && self.holding_gun();
        if inspecting {
            self.start_inspect();
        }
        // Double tap forward to sprint.
        if forward && !self.input.keys.contains(&code) {
            self.input.tap_forward(self.clock.time);
        }
        if stop_sprint {
            self.input.w_sprint = false;
        }
        if drop {
            let all = self.input.ctrl();
            self.drop_held(all);
        }
        if fly && self.creative() && !inspecting {
            self.me.body.flying = !self.me.body.flying;
        }
        if jump && self.creative() && self.input.tap_jump(self.clock.time) {
            self.me.body.flying = !self.me.body.flying;
        }
        // Last: these leave the game screen.
        if chat {
            self.open_chat("");
        } else if command {
            self.open_chat("/");
        }
    }

    fn open_chat(&mut self, prefix: &str) {
        self.chat.open(prefix);
        self.screen = Screen::Chat;
        self.set_grab(false);
        self.input.keys.clear();
    }

    pub(super) fn set_grab(&mut self, grab: bool) {
        let grab = grab && !self.test.any();
        if !grab {
            self.input.release();
        }
        let size = self.gfx.window.inner_size();
        let center = PhysicalPosition::new(size.width as f64 / 2.0, size.height as f64 / 2.0);
        if grab {
            let _ = self
                .gfx.window
                .set_cursor_grab(CursorGrabMode::Locked)
                .or_else(|_| self.gfx.window.set_cursor_grab(CursorGrabMode::Confined));
            self.gfx.window.set_cursor_visible(false);
        } else {
            let _ = self.gfx.window.set_cursor_grab(CursorGrabMode::None);
            self.gfx.window.set_cursor_visible(true);
            if self.input.cursor_grabbed {
                let _ = self.gfx.window.set_cursor_position(center);
                self.ui.mouse = Vec2::new(center.x as f32, center.y as f32);
            }
        }
        self.input.cursor_grabbed = grab;
        self.input.mouse_delta = Vec2::ZERO;
        self.input.left_down = false;
        self.input.right_down = false;
        self.me.aim.mining = None;
    }

    pub(super) fn toggle_fullscreen(&mut self) {
        self.settings.fullscreen = !self.settings.fullscreen;
        self.gfx.window.set_fullscreen(if self.settings.fullscreen {
            Some(Fullscreen::Borderless(None))
        } else {
            None
        });
    }
}
