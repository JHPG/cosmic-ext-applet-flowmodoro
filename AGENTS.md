# Working on this applet

- Code is split into modules under `src/`, one concern per file, `main.rs` only wires the
  applet to them. Don't put new code in `main.rs`; give it a module. `cargo fmt`, `just test`,
  `just build`.
- Every module carries its own `#[cfg(test)] mod tests` next to the code it tests.
- **Never change how the session runs things.** No systemd units, no edits to the COSMIC
  config, no autostart or launcher changes, nothing written outside the applet's own files
  under `~/.local`, and don't install packages to make debugging easier. If the panel's
  respawn looks broken, report it and leave it alone.
- **Killing, restarting and spawning processes is fine for debugging** — the panel, the
  applet processes, a throwaway run of the binary. Say when you are cycling the panel, since
  that restarts every applet in the session.
- **Never start `cosmic-panel` by hand.** `cosmic-session` starts its own a few minutes after
  it dies; a hand-started one has no parent session and ends up as a second bar. After
  `pkill -x cosmic-panel`, wait for the session's panel instead of starting one.
- The panel runs `~/.local/bin/cosmic-ext-applet-flowmodoro`, not `target/`, so a change is
  invisible until it is installed *and* the panel respawns the applet processes. Killing one
  applet process is not enough — the panel only respawns them at startup.
- **After implementing a change, run `just reload`** (install + `pkill -x cosmic-panel`) so the
  change is live. This is allowed without asking; say when you do it, since it restarts every
  applet in the session. Use `just install` alone only when a reload isn't needed; pair either
  with `cargo test` and a look at the state file.
- Strings: the `Tr` table in `src/tr.rs` plus a `pick()` branch; keep `data/*.desktop` and
  `data/*.metainfo.xml` translations in sync when adding a language.
- Keep `README.md` in sync: when a feature or substantial change lands, update the README in
  the same pass (new behaviour, table rows, install steps, screenshots under `data/`).
- The shared state file (`$XDG_RUNTIME_DIR/flowmodoro`, one process per monitor) plus the
  one-shot markers `flowmodoro.over` / `flowmodoro.late` are the cross-process protocol.
  A live check is possible without installing: run `target/release/<name>` by hand and poke
  that file, but a standalone copy is NOT the panel — it proves logic, not the rendered UI.
