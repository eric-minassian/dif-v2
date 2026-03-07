mod config;
mod font;
mod session;

pub mod view;

pub use config::{Rgb, StyleRun, TerminalConfig};
pub use font::{default_terminal_font, default_terminal_font_features};
pub use session::{CursorShape, TerminalSession};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod vt_tests;
