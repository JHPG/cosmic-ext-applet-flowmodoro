//! The history file: every finished focus and break, one line each, kept forever
//! so the Stats tab survives logout. Pure format and IO; `stats.rs` reads it.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Entry {
    Focus {
        start: SystemTime,
        focus: Duration,
    },
    Break {
        start: SystemTime,
        earned: Duration,
        taken: Duration,
    },
}

fn millis(t: SystemTime) -> u128 {
    t.duration_since(UNIX_EPOCH).unwrap_or_default().as_millis()
}

pub(crate) fn encode(e: &Entry) -> String {
    match e {
        Entry::Focus { start, focus } => {
            format!("focus {} {}", millis(*start), focus.as_millis())
        }
        Entry::Break {
            start,
            earned,
            taken,
        } => format!(
            "break {} {} {}",
            millis(*start),
            earned.as_millis(),
            taken.as_millis()
        ),
    }
}

fn at(s: &str) -> Option<SystemTime> {
    Some(UNIX_EPOCH + Duration::from_millis(s.parse().ok()?))
}

fn dur(s: &str) -> Option<Duration> {
    Some(Duration::from_millis(s.parse().ok()?))
}

/// None for anything that isn't a whole, well-formed line.
pub(crate) fn decode(line: &str) -> Option<Entry> {
    let fields: Vec<&str> = line.split_whitespace().collect();
    match fields.as_slice() {
        ["focus", start, focus] => Some(Entry::Focus {
            start: at(start)?,
            focus: dur(focus)?,
        }),
        ["break", start, earned, taken] => Some(Entry::Break {
            start: at(start)?,
            earned: dur(earned)?,
            taken: dur(taken)?,
        }),
        _ => None,
    }
}

/// Decodes a whole file, skipping lines it can't read.
fn parse(text: &str) -> Vec<Entry> {
    text.lines().filter_map(decode).collect()
}

fn non_empty(var: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

pub(crate) fn path() -> Option<PathBuf> {
    let data =
        non_empty("XDG_DATA_HOME").or_else(|| non_empty("HOME").map(|h| h.join(".local/share")))?;
    Some(data.join("flowmodoro").join("history"))
}

/// Errors are ignored, as in `state.rs`: a lost history line must not break the timer.
pub(crate) fn append(e: &Entry) {
    let Some(path) = path() else { return };
    if let Some(dir) = path.parent() {
        drop(fs::create_dir_all(dir));
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        drop(writeln!(file, "{}", encode(e)));
    }
}

/// Missing file = no history yet.
pub(crate) fn load() -> Vec<Entry> {
    path()
        .and_then(|p| fs::read_to_string(p).ok())
        .map(|text| parse(&text))
        .unwrap_or_default()
}

/// Modification time of the history file: lets the Stats tab skip re-reading it.
pub(crate) fn modified() -> Option<SystemTime> {
    fs::metadata(path()?).ok()?.modified().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(s)
    }

    #[test]
    fn each_entry_survives_a_round_trip_through_its_line() {
        let focus = Entry::Focus {
            start: at(1_700_000_000),
            focus: Duration::from_millis(1500),
        };
        let brk = Entry::Break {
            start: at(1_700_000_100),
            earned: Duration::from_secs(300),
            taken: Duration::from_secs(420),
        };
        for e in [focus, brk] {
            assert_eq!(decode(&encode(&e)), Some(e));
        }
    }

    #[test]
    fn garbage_and_truncated_lines_are_skipped() {
        let text = "focus 1000 2000\nnonsense\nbreak 1000 300000\n\nbreak 3000 1000 2000 9\n";
        assert_eq!(
            parse(text),
            vec![Entry::Focus {
                start: at(1),
                focus: Duration::from_secs(2),
            }]
        );
    }
}
