//! UI strings: one table per language, picked from the session locale.

use std::sync::LazyLock;

/// UI strings. ponytail: plain struct per language; move to Fluent (i18n-embed)
/// if outside translators start contributing. A missing translation is a compile
/// error, so no completeness test is needed; what the struct cannot do is carry
/// a computed value, which is why the two number fields take one.
pub(crate) struct Tr {
    pub(crate) ready: &'static str,
    pub(crate) idle_hint: fn(u32) -> String,
    pub(crate) start: &'static str,
    pub(crate) focus: &'static str,
    pub(crate) earned: fn(&str) -> String,
    pub(crate) reset: &'static str,
    pub(crate) take_break: &'static str,
    pub(crate) brk: &'static str,
    pub(crate) rest: fn(&str) -> String,
    pub(crate) late: fn(&str) -> String,
    pub(crate) skip: &'static str,
    pub(crate) break_started: fn(&str, &str) -> String,
    pub(crate) break_over: &'static str,
    pub(crate) break_over_body: &'static str,
    pub(crate) still_away: &'static str,
    pub(crate) still_away_body: fn(u64) -> String,
}

const EN: Tr = Tr {
    ready: "Ready",
    idle_hint: |r| format!("Focus as long as you like; break = focus ÷ {r}"),
    start: "Start focus",
    focus: "Focus",
    earned: |b| format!("Break earned: {b}"),
    reset: "Reset",
    take_break: "Take a break",
    brk: "Break",
    rest: |e| format!("Rest · {e} so far"),
    late: |o| format!("Late by: {o}"),
    skip: "End break",
    break_started: |f, b| format!("Focused {f} → {b} break"),
    break_over: "Break over",
    break_over_body: "Time to get back to focus",
    still_away: "Still on break",
    // ponytail: no plural rule; wrong for a 1-minute nudge, add one if it gets that small
    still_away_body: |m| format!("The break ended {m} minutes ago"),
};

const PT: Tr = Tr {
    ready: "Pronto",
    idle_hint: |r| format!("Foque o quanto quiser; pausa = foco ÷ {r}"),
    start: "Iniciar foco",
    focus: "Foco",
    earned: |b| format!("Pausa acumulada: {b}"),
    reset: "Reset",
    take_break: "Descansar",
    brk: "Pausa",
    rest: |e| format!("Descanse · {e} até agora"),
    late: |o| format!("Atraso: {o}"),
    skip: "Encerrar pausa",
    break_started: |f, b| format!("Foco {f} → pausa de {b}"),
    break_over: "Pausa acabou",
    break_over_body: "Hora de voltar ao foco",
    still_away: "Ainda na pausa",
    still_away_body: |m| format!("A pausa acabou há {m} minutos"),
};

const ES: Tr = Tr {
    ready: "Listo",
    idle_hint: |r| {
        format!("Concéntrate todo el tiempo que quieras; descanso = concentración ÷ {r}")
    },
    start: "Empezar a concentrarse",
    focus: "Concentración",
    earned: |b| format!("Descanso acumulado: {b}"),
    reset: "Reiniciar",
    take_break: "Descansar",
    brk: "Descanso",
    rest: |e| format!("Descanse · {e} hasta ahora"),
    late: |o| format!("Retraso: {o}"),
    skip: "Terminar el descanso",
    break_started: |f, b| format!("Concentración {f} → descanso de {b}"),
    break_over: "Descanso terminado",
    break_over_body: "Es hora de volver a concentrarse",
    still_away: "Todavía en descanso",
    still_away_body: |m| format!("El descanso terminó hace {m} minutos"),
};

/// Language from a POSIX locale ("pt_BR.UTF-8", "es_ES.UTF-8"); English otherwise.
fn pick(locale: &str) -> &'static Tr {
    if locale.starts_with("pt") {
        &PT
    } else if locale.starts_with("es") {
        &ES
    } else {
        &EN
    }
}

pub(crate) fn tr() -> &'static Tr {
    static T: LazyLock<&Tr> = LazyLock::new(|| {
        let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .filter_map(|v| std::env::var(v).ok())
            .find(|v| !v.is_empty())
            .unwrap_or_default();
        pick(&locale)
    });
    *T
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase::{BREAK_RATIO, LATE_NUDGE};

    #[test]
    fn the_language_comes_from_the_locale() {
        assert_eq!(pick("pt_BR.UTF-8").focus, "Foco");
        assert_eq!(pick("pt_PT").focus, "Foco");
        assert_eq!(pick("es_ES.UTF-8").focus, "Concentración");
        assert_eq!(pick("es_MX").skip, "Terminar el descanso");
        assert_eq!(pick("en_US.UTF-8").focus, "Focus");
        assert_eq!(pick("").focus, "Focus");
    }

    #[test]
    fn every_language_quotes_the_real_constants() {
        let nudge = LATE_NUDGE.as_secs() / 60;
        for t in [&EN, &PT, &ES] {
            assert!(
                (t.idle_hint)(BREAK_RATIO).contains(&BREAK_RATIO.to_string()),
                "the idle hint doesn't quote the break ratio"
            );
            assert!(
                (t.still_away_body)(nudge).contains(&nudge.to_string()),
                "the still-away body doesn't quote the nudge"
            );
        }
    }
}
