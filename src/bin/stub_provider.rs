//! Standalone stub model provider for driving the real desktop app.
//!
//! Runs `surface::stub::StubServer` (theme-aware) as a separate process so the
//! Tauri app can be pointed at it via `AIOS_CONFIG` and exercised end to end
//! without a real model or network. Prints the listening port on stdout and
//! serves until terminated.

use std::io::Write;

fn main() {
    let stub = aios::surface::stub::StubServer::spawn_themed();
    let mut stdout = std::io::stdout();
    let _ = writeln!(stdout, "stub provider listening on 127.0.0.1:{}", stub.port);
    let _ = stdout.flush();
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}
