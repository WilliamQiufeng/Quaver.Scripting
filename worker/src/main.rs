
pub fn apply_sandbox() -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    {
        lib::linux::apply_sandbox()
    }

    #[cfg(not(target_os = "linux"))]
    Ok(())
}

fn main() -> anyhow::Result<()> {
    apply_sandbox()?;
    _ = lib::test();
    println!("Hello, world!");
    Ok(())
}
