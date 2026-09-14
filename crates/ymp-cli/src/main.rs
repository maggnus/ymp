#![forbid(unsafe_code)]

use std::env;
use std::ffi::{OsStr, OsString};
use std::process::ExitCode;

use ymp_runtime::{ApplicationMetadata, application_metadata};

fn main() -> ExitCode {
    let arguments: Vec<OsString> = env::args_os().skip(1).collect();
    match command(&arguments) {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}

fn command(arguments: &[OsString]) -> Result<String, String> {
    let metadata = application_metadata();
    match arguments {
        [] => Ok(help(metadata)),
        [argument] if argument == OsStr::new("--help") => Ok(help(metadata)),
        [argument] if argument == OsStr::new("--version") => {
            Ok(format!("{} {}\n", metadata.name(), metadata.version()))
        }
        [argument] => Err(format!(
            "error: unsupported argument '{}'\n\n{}",
            argument.to_string_lossy(),
            usage(metadata)
        )),
        _ => Err(format!(
            "error: expected at most one argument, received {}\n\n{}",
            arguments.len(),
            usage(metadata)
        )),
    }
}

fn help(metadata: ApplicationMetadata) -> String {
    format!(
        "{} {}\n\n{}\n\n{}\n",
        metadata.name(),
        metadata.version(),
        usage(metadata),
        metadata.capability_limit()
    )
}

fn usage(metadata: ApplicationMetadata) -> String {
    format!("Usage: {} [--help | --version]", metadata.name())
}
