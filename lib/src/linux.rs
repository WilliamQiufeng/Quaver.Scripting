use landlock::{ABI, Access, AccessFs, Ruleset, RulesetAttr};
use seccompiler::{BpfProgram, SeccompAction, SeccompFilter, TargetArch, apply_filter};
use std::collections::BTreeMap;

pub fn apply_sandbox() -> anyhow::Result<()> {
    restrict_filesystem_with_landlock()?;
    install_seccomp()?;

    Ok(())
}

fn restrict_filesystem_with_landlock() -> anyhow::Result<()> {
    let abi = ABI::V1;

    let handled = AccessFs::from_all(abi);

    let status = Ruleset::default()
        .handle_access(handled)?
        .create()?
        .restrict_self()?;

    eprintln!("Landlock status: {status:?}");

    Ok(())
}

fn current_arch() -> TargetArch {
    #[cfg(target_arch = "x86_64")]
    {
        return TargetArch::x86_64;
    }

    #[cfg(target_arch = "aarch64")]
    {
        return TargetArch::aarch64;
    }

    #[cfg(target_arch = "riscv64")]
    {
        return TargetArch::riscv64;
    }

    #[cfg(not(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "riscv64"
    )))]
    compile_error!("Unsupported architecture for seccompiler");
}

#[cfg(target_os = "linux")]
fn install_seccomp() -> anyhow::Result<()> {
    let mut rules = BTreeMap::new();

    // Exact syscall-number API depends on seccompiler version.
    //
    // Deny things plugins should never need:
    add_deny(&mut rules, libc::SYS_ptrace)?;
    add_deny(&mut rules, libc::SYS_mount)?;
    add_deny(&mut rules, libc::SYS_umount2)?;
    add_deny(&mut rules, libc::SYS_reboot)?;

    // If network must be completely unavailable:
    add_deny(&mut rules, libc::SYS_socket)?;
    add_deny(&mut rules, libc::SYS_connect)?;
    add_deny(&mut rules, libc::SYS_bind)?;
    add_deny(&mut rules, libc::SYS_listen)?;

    // Optionally prevent execution of other programs:
    add_deny(&mut rules, libc::SYS_execve)?;
    #[cfg(target_arch = "x86_64")]
    add_deny(&mut rules, libc::SYS_execveat)?;

    let filter = SeccompFilter::new(
        rules,
        SeccompAction::Allow,
        SeccompAction::Errno(libc::EPERM as u32),
        current_arch(),
    )?;

    let program: BpfProgram = filter.try_into()?;

    apply_filter(&program)?;

    Ok(())
}

fn add_deny(
    map: &mut std::collections::BTreeMap<i64, Vec<seccompiler::SeccompRule>>,
    syscall: i64,
) -> anyhow::Result<()> {
    map.insert(syscall, vec![]);
    Ok(())
}
