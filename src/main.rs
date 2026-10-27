mod app;
mod dictation;
mod leitner;
mod player;
mod review;
mod scene;
mod srt;
mod state;
mod tui;
mod ui;
mod words;

#[cfg(feature = "audio")]
mod audio;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::app::App;
use crate::review::Review;
use crate::state::State;
use crate::words::{SceneMark, WordsFile};

const USAGE: &str = "\
listen — intensive listening practice in the terminal

Usage:
  listen play <file.m4a>    study an episode (needs <file>.srt next to it)
  listen play --whole <file.m4a>
                            treat the whole episode as one scene: listen
                            straight through instead of scene by scene
  listen review [--all]     review saved expressions (--all: ignore due dates)
  listen cues <file.srt>    print sentence numbers used by words.json
  listen check <file.srt> <file.words.json>
                            validate a words.json against its subtitles

Press ? inside the app for keys.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["play", file] => play(Path::new(file), false),
        ["play", "--whole", file] | ["play", file, "--whole"] => play(Path::new(file), true),
        ["review"] => review(false),
        ["review", "--all"] => review(true),
        ["cues", srt] => print_cues(Path::new(srt)),
        ["check", srt, words] => check_words(Path::new(srt), Path::new(words)),
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

fn check_words(srt_path: &Path, words_path: &Path) -> Result<(), String> {
    let cues = srt::parse(&read(srt_path)?);
    let words = WordsFile::parse(&read(words_path)?)
        .map_err(|e| format!("invalid {}: {e}", words_path.display()))?;
    let problems = words::check(&words, &cues);
    for p in &problems {
        eprintln!("warning: {p}");
    }
    println!(
        "{} items, {} scenes, {} warnings",
        words.items.len(),
        words.scenes.len(),
        problems.len()
    );
    Ok(())
}

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

fn load_state() -> Result<(State, PathBuf), String> {
    let path =
        state::state_path().ok_or("cannot find a data directory; set LL_HOME to choose one")?;
    Ok((State::load(&path)?, path))
}

fn play(audio: &Path, whole: bool) -> Result<(), String> {
    if !audio.is_file() {
        return Err(format!("audio file not found: {}", audio.display()));
    }
    let srt_path = audio.with_extension("srt");
    let cues = srt::parse(&read(&srt_path)?);
    if cues.is_empty() {
        return Err(format!("no subtitles found in {}", srt_path.display()));
    }
    let words_path = audio.with_extension("words.json");
    let (words, words_note) = if words_path.is_file() {
        let words = WordsFile::parse(&read(&words_path)?)
            .map_err(|e| format!("invalid {}: {e}", words_path.display()))?;
        (words, None)
    } else {
        let note = "没有 words.json：预习页为空（可用 scripts/annotate.sh 生成）";
        (WordsFile::default(), Some(note.to_string()))
    };
    let mut words = words;
    if whole {
        words.scenes = vec![SceneMark {
            start: 1,
            title: Some("整集".into()),
        }];
    }
    let (state, state_path) = load_state()?;

    let key = std::fs::canonicalize(audio)
        .unwrap_or_else(|_| audio.to_path_buf())
        .display()
        .to_string();
    let title = audio
        .file_name()
        .map_or(key.clone(), |n| n.to_string_lossy().into_owned());
    let duration = cues.last().map(|c| c.end_ms);
    let (player, audio_note) = match player::open(audio, duration) {
        Ok(p) => (p, None),
        Err(e) => (
            Box::new(player::NullPlayer::new(duration)) as Box<dyn player::Player>,
            Some(format!("音频不可用，静音运行：{e}")),
        ),
    };

    let mut app = App::new(title, key, cues, words, state, Some(state_path), player);
    if let Some(note) = audio_note.or(words_note) {
        app.message = note;
    }
    tui::run(&mut app).map_err(|e| format!("terminal error: {e}"))
}

fn review(all: bool) -> Result<(), String> {
    let (state, state_path) = load_state()?;
    let opener: review::Opener = Box::new(player::open);
    let mut screen = Review::new(state, Some(state_path), leitner::today(), all, opener);
    tui::run(&mut screen).map_err(|e| format!("terminal error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_fixture_is_consistent() {
        let cues = srt::parse(include_str!("../fixtures/sample.srt"));
        let words = WordsFile::parse(include_str!("../fixtures/sample.words.json")).unwrap();
        assert_eq!(cues.len(), 45);
        assert!(words::check(&words, &cues).is_empty());
        let scenes = scene::scenes_for(&cues, &words.scenes);
        assert_eq!(scenes.len(), 2);
        assert_eq!(scenes[1].start, 24);
        // Without scene marks, 45 sentences are too few to split.
        assert_eq!(scene::split_by_gaps(&cues).len(), 1);
    }
}
