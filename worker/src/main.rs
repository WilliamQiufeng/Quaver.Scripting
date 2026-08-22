use std::{
    env::args,
    io::{Error, ErrorKind},
    time::Instant,
};

use anyhow::bail;
use lib::shared_memory;
use std::io::Write;

const BUILD_PROFILE: &str = if cfg!(debug_assertions) {
    "dev"
} else {
    "release"
};

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

    println!("worker build profile: {BUILD_PROFILE}");
    apply_sandbox()?;
    lib::test()?;
    measure_writes::<_, 32_767>(&mut mem, 4)?;
    measure_writes::<_, 4>(&mut mem, 32_767)?;
    Ok(())
}

fn measure_writes<W: Write, const BATCH_SIZE: usize>(
    writer: &mut W,
    write_count: usize,
) -> std::io::Result<()> {
    let byte = [0x48; BATCH_SIZE];
    let start = Instant::now();

    for _ in 0..write_count {
        let written = writer.write(&byte)?;
        if written != byte.len() {
            return Err(Error::new(
                ErrorKind::WriteZero,
                format!("duplex write wrote {written} bytes"),
            ));
        }
    }

    let elapsed = start.elapsed();
    let average = elapsed.as_nanos() as f64 / write_count as f64;
    let average_per_byte = average / byte.len() as f64;

    println!(
        "write: {average:.2} ns/ {BATCH_SIZE} bytes batch ({average_per_byte:.2} ns/B) over {write_count} writes (total: {elapsed:?})"
    );

    Ok(())
}
