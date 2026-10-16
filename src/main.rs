mod dictation;
mod leitner;
mod player;
mod scene;
mod srt;
mod state;
mod words;

#[cfg(feature = "audio")]
mod audio;

use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "\
listen — intensive listening practice in the terminal

Usage:
  listen play <file.m4a>    study an episode (needs <file>.srt next to it)
  listen review [--all]     review saved expressions (--all: ignore due dates)
  listen cues <file.srt>    print sentence numbers used by words.json

Press ? inside the app for keys.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["cues", srt] => print_cues(Path::new(srt)),
        ["-h" | "--help" | "help"] => {
            println!("{USAGE}");
            Ok(())
        }
        ["-V" | "--version"] => {
            println!("listen {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        _ => Err(USAGE.to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn print_cues(path: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    for (i, cue) in srt::parse(&text).iter().enumerate() {
        println!("{}|{}", i + 1, cue.one_line());
    }
    Ok(())
}
