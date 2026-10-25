#[cfg(windows)]
mod desktop;
#[cfg(windows)]
mod native;
#[cfg(any(windows, test))]
mod observation;
#[cfg(any(windows, test))]
mod protocol;
#[cfg(windows)]
mod run_capture;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    return native::run();
    #[cfg(not(windows))]
    Err("This spike requires an interactive Windows desktop. No measurements were made.".into())
}
