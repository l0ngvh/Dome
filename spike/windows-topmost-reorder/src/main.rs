#[cfg(windows)]
mod native;

#[cfg(windows)]
fn main() {
    if let Err(error) = native::run() {
        eprintln!("windows-topmost-reorder: {error}");
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!(
        "windows-topmost-reorder requires Windows. Pure correlation tests run with cargo test --lib."
    );
    std::process::exit(1);
}
