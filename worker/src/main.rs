use std::{env::args, fs::{File, OpenOptions}};

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
    let file = OpenOptions::new().read(true).write(true).open(path)?;

    let mut mem = shared_memory::SharedMemoryInstance::from_file(&file, size)?;

    apply_sandbox()?;
    lib::test()?;
    writeln!(mem, "Hi")?;
    println!("Hello, world!");
    Ok(())
}
