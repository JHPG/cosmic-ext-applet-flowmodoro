# Working on this applet

- Code is split into modules under `src/`, one concern per file, `main.rs` only wires the
  applet to them. Don't put new code in `main.rs`; give it a module. `cargo fmt`, `just test`,
  `just build`.
- Every module carries its own `#[cfg(test)] mod tests` next to the code it tests.
- **Never change how the session runs things.** No systemd units, no edits to the COSMIC
  config, no autostart or launcher changes, nothing written outside the applet's own files
  under `~/.local`. If the panel's respawn looks broken, report it and leave it alone —
  don't fix it by changing the desktop.
- **Restarting the panel is allowed** when it's the only way to check a change. `just reload`
  installs and kills `cosmic-panel`, cycling every applet in the session, so say that before
  running it. The respawn is unreliable here — it has come back as zero panels, and once as
  two stacked panels — so afterwards confirm exactly one panel is running and report what
  you find.
- The panel runs `~/.local/bin/cosmic-ext-applet-flowmodoro`, not `target/`, so a change is
  invisible until it is installed *and* the panel respawns the applet processes. Killing one
  applet process is not enough — the panel only respawns them at startup.
- `just install` alone updates the binary without touching the session; pair it with
  `cargo test` and a look at the state file when a reload isn't needed.
- Strings: the `Tr` table in `src/tr.rs` plus a `pick()` branch; keep `data/*.desktop` and
  `data/*.metainfo.xml` translations in sync when adding a language.
- Keep `README.md` in sync: when a feature or substantial change lands, update the README in
  the same pass (new behaviour, table rows, install steps, screenshots under `data/`).
- The shared state file (`$XDG_RUNTIME_DIR/flowmodoro`, one process per monitor) plus the
  one-shot markers `flowmodoro.over` / `flowmodoro.late` are the cross-process protocol.
  A live check is possible without installing: run `target/release/<name>` by hand and poke
  that file, but a standalone copy is NOT the panel — it proves logic, not the rendered UI.
