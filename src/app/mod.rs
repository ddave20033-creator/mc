//! The program around the game: what it is started with and keeps between runs, and what
//! shows before the game does.
//!
//! - `settings`: the options (`options.txt`), with the key binds of `keys`.
//! - `keys`: the rebindable keys, their defaults and names.
//! - `lang`: the UI's translations (English, Hungarian).
//! - `splash`: the start-up splash (the logo filling up) while the game gets ready.
//! - `stats`: the system statistics of the F3 screen (CPU, RAM, GPU).
//! - `devtools`: the command-line tools run instead of the game (`--map`, `--stats`, `--sizes`).

pub mod devtools;
pub mod keys;
pub mod lang;
pub mod settings;
pub mod splash;
pub mod stats;
