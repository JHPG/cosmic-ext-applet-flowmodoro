//! The shared state every applet instance reads and writes, and the one-shot
//! bells beside it. The mechanism is settled in `docs/adr/0001`; read it before
//! proposing to change how the instances find each other.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::phase::{Phase, LATE_NUDGE};
use crate::tr::tr;

/// The state file and the bells beside it are siblings of one name.
const STATE_FILE: &str = "flowmodoro";

// Panel runs one applet process per monitor; they share state through this file.
// Missing file = Idle.

fn state_dir() -> PathBuf {
    let mut dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    // inside Flatpak only $XDG_RUNTIME_DIR/app/<id> is shared between instances
    if let Some(id) = std::env::var_os("FLATPAK_ID") {
        dir = dir.join("app").join(id);
    }
    dir
}

/// The shared directory may not exist yet (Flatpak creates it lazily), and a
/// missing directory means state and bells fail silently.
fn ensure_state_dir() {
    drop(fs::create_dir_all(state_dir()));
}

fn state_path() -> PathBuf {
    state_dir().join(STATE_FILE)
}

/// One-shot bell: a marker file whose existence records that this notification
/// already fired, so N applet instances produce one notification.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Bell {
    Over,
    Late,
}

impl Bell {
    const ALL: [Bell; 2] = [Bell::Over, Bell::Late];

    fn name(self) -> &'static str {
        match self {
            Bell::Over => "over",
            Bell::Late => "late",
        }
    }

    fn path(self) -> PathBuf {
        state_dir().join(format!("{STATE_FILE}.{}", self.name()))
    }

    /// true exactly once per break: only the instance that creates the marker
    /// gets to notify.
    pub(crate) fn fires(self) -> bool {
        ensure_state_dir();
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.path())
            .is_ok()
    }

    /// a new break starts with fresh bells
    pub(crate) fn clear_all() {
        for b in Bell::ALL {
            drop(fs::remove_file(b.path()));
        }
    }

    pub(crate) fn copy(self) -> (&'static str, String) {
        let t = tr();
        match self {
            Bell::Over => (t.break_over, t.break_over_body.into()),
            Bell::Late => (t.still_away, (t.still_away_body)(LATE_NUDGE.as_secs() / 60)),
        }
    }
}

fn encode(p: Phase) -> Option<String> {
    let ms = |t: SystemTime| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
    match p {
        Phase::Idle => None,
        Phase::Focus(t) => Some(format!("focus {}", ms(t))),
        Phase::Break(s, e) => Some(format!("break {} {}", ms(s), ms(e))),
    }
}

fn decode(s: &str) -> Phase {
    let at = |v: &str| {
        v.parse()
            .ok()
            .map(|ms: u64| UNIX_EPOCH + Duration::from_millis(ms))
    };
    let mut w = s.trim().split(' ');
    match (
        w.next().unwrap_or_default(),
        w.next().and_then(at),
        w.next().and_then(at),
    ) {
        ("focus", Some(t), None) => Phase::Focus(t),
        ("break", Some(s), Some(e)) => Phase::Break(s, e),
        _ => Phase::Idle,
    }
}

pub(crate) fn load() -> Phase {
    fs::read_to_string(state_path()).map_or(Phase::Idle, |s| decode(&s))
}

pub(crate) fn save(p: Phase) {
    let path = state_path();
    match encode(p) {
        None => drop(fs::remove_file(path)),
        Some(s) => {
            // write + rename so the other instance never reads a half-written file
            let tmp = path.with_extension(std::process::id().to_string());
            ensure_state_dir();
            if fs::write(&tmp, s).is_ok() {
                let _ = fs::rename(tmp, path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_phase_survives_the_round_trip() {
        let t = UNIX_EPOCH + Duration::from_millis(1_700_000_000_123);
        for p in [
            Phase::Focus(t),
            Phase::Break(t, t + Duration::from_secs(600)),
        ] {
            assert_eq!(decode(&encode(p).unwrap()), p);
        }
        assert_eq!(encode(Phase::Idle), None);
        assert_eq!(decode("break 1700000000123"), Phase::Idle);
        assert_eq!(decode("focus 1 2"), Phase::Idle);
        assert_eq!(decode("garbage"), Phase::Idle);
    }

    #[test]
    fn bells_are_one_shot_per_break() {
        let dir = std::env::temp_dir().join(format!("flowmodoro-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("XDG_RUNTIME_DIR", &dir);
        assert!(Bell::Over.fires());
        assert!(!Bell::Over.fires());
        assert!(Bell::Late.fires());
        assert!(!Bell::Late.fires());
        // a new break re-arms both
        Bell::clear_all();
        assert!(Bell::Over.fires());
        assert!(Bell::Late.fires());
        drop(std::fs::remove_dir_all(&dir));
    }
}
