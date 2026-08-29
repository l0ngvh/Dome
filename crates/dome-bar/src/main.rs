fn main() -> anyhow::Result<()> {
    let up = dome_ipc::DomeClient.ping();
    eprintln!("dome-bar: dome socket reachable = {up}");
    Ok(())
}
