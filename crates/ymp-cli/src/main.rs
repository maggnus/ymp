#![forbid(unsafe_code)]

use std::env;
use std::ffi::OsString;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

fn main() -> ExitCode {
    let arguments: Vec<OsString> = env::args_os().skip(1).collect();
    #[cfg(target_os = "linux")]
    if arguments
        .first()
        .is_some_and(|argument| argument == "__ymp_internal_check_sandbox")
    {
        return ExitCode::from(ymp_runtime::run_linux_check_sandbox(&arguments[1..]));
    }
    #[cfg(target_os = "linux")]
    if arguments
        .first()
        .is_some_and(|argument| argument == "__ymp_internal_check_sandbox_stage2")
    {
        return ExitCode::from(ymp_runtime::run_linux_check_sandbox_stage2(&arguments[1..]));
    }
    let interrupted = Arc::new(AtomicBool::new(false));
    let _sigint =
        match signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&interrupted)) {
            Ok(registration) => registration,
            Err(error) => {
                eprintln!("error: cannot install SIGINT handler: {error}");
                return ExitCode::from(1);
            }
        };
    let output = ymp_cli::command_with_interrupt(&arguments, interrupted.as_ref());
    print!("{}", output.stdout());
    eprint!("{}", output.stderr());
    ExitCode::from(output.exit_code())
}
