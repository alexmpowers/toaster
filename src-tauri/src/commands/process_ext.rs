//! Process-spawn helpers shared by every shell-out site.
//!
//! On Windows, `std::process::Command::new("ffmpeg" | "ffprobe" | "cmd")`
//! flashes a black console window for ~50 ms because the spawned child
//! inherits a console. With waveform decode + ffprobe + cleanup all
//! firing on routine UI actions (load media, scroll timeline, click
//! Remove Silence) the user sees seemingly-random pop-ups.
//!
//! `CREATE_NO_WINDOW` (0x08000000) suppresses the console without
//! affecting stdout/stderr piping, which is exactly what we want for
//! `ffmpeg` / `ffprobe`. On non-Windows targets the call is a no-op so
//! the same fluent chain compiles everywhere.
//!
//! Single source of truth — every production `Command::new` for an
//! external tool MUST go through `.no_console_window()`. Tests are
//! exempt (they only run under `cargo test` and a flashing window
//! during tests is harmless).

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub trait NoConsoleWindow {
    fn no_console_window(&mut self) -> &mut Self;
}

impl NoConsoleWindow for std::process::Command {
    #[cfg(windows)]
    fn no_console_window(&mut self) -> &mut Self {
        use std::os::windows::process::CommandExt;
        self.creation_flags(CREATE_NO_WINDOW)
    }

    #[cfg(not(windows))]
    fn no_console_window(&mut self) -> &mut Self {
        self
    }
}
