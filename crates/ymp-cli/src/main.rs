#![forbid(unsafe_code)]

use std::env;
use std::ffi::OsString;
use std::process::ExitCode;

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
    let output = ymp_cli::command(&arguments);
    print!("{}", output.stdout());
    eprint!("{}", output.stderr());
    ExitCode::from(output.exit_code())
}
