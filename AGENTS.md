# Working on this applet

- All code is `src/main.rs`. `cargo fmt`, `just test`, `just build`.
- **Always finish with `just reload`** (install + restart the panel). The panel runs
  `~/.local/bin/cosmic-ext-applet-flowmodoro`, not `target/`, so a change is invisible
  until it is installed *and* the panel respawns the applet processes. Killing one applet
  process is not enough — the panel only respawns them at startup, so restart the panel.
- A panel restart cycles every applet in the session, so mention it when reloading.
- Strings: `Tr` in `src/main.rs` plus a `pick()` branch; keep `data/*.desktop` and
  `data/*.metainfo.xml` translations in sync when adding a language.
- The shared state file (`$XDG_RUNTIME_DIR/flowmodoro`, one process per monitor) plus the
  one-shot markers `flowmodoro.over` / `flowmodoro.late` are the cross-process protocol.
  A live check is possible without installing: run `target/release/<name>` by hand and poke
  that file, but a standalone copy is NOT the panel — it proves logic, not the rendered UI.
