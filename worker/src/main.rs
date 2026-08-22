use std::{env::args, thread::sleep, time::Duration};

use anyhow::bail;
use lib::shared_memory;
use std::io::Write;

pub fn apply_sandbox() -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    {
        lib::linux::apply_sandbox()
    }

    #[cfg(not(target_os = "linux"))]
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args = args().collect::<Vec<_>>();
    if args.len() < 3 {
        bail!("Invalid argument");
    }
    let path = &args[1];
    let size = usize::from_str_radix(&args[2], 10)?;

    let mut mem = shared_memory::duplex::DuplexInstance::from_file(path, size)?;

    apply_sandbox()?;
    lib::test()?;
    println!("Hello, world!");
    loop {
        writeln!(mem, "Hi")?;
        sleep(Duration::from_secs(1));
    }
}
