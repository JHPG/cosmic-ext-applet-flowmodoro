# Publishing

## GitHub

1. Create the repository `JHPG/cosmic-ext-applet-flowmodoro` and push `main`.
2. Keep the Store screenshots in `data/` up to date (`screenshot.png`, `screenshot-edit.png`,
   `screenshot-break.png`); the metainfo links to them on `main`.
3. Tag the release: `git tag v0.1.0 && git push --tags`, then create a GitHub release.

## COSMIC Store

Panel applets are not accepted on Flathub. They are distributed through the
[COSMIC Flatpak repository](https://github.com/pop-os/cosmic-flatpak), which the COSMIC Store
reads. The `<provides><id>com.system76.CosmicApplet</id></provides>` entry in the metainfo
places the app in the Store's *Applets* section.

1. Bump `version` in `Cargo.toml`, add a `<release>` entry in
   `data/io.github.jhpg.cosmic-ext-applet-flowmodoro.metainfo.xml` and commit.
2. Run `just validate`, then `just flatpak-sources` if `Cargo.lock` changed.
3. Fork `pop-os/cosmic-flatpak` and add `app/io.github.jhpg.cosmic-ext-applet-flowmodoro/` with:
   - `cargo-sources.json` (from `flatpak/`)
   - the manifest from `flatpak/`, with the `dir` source replaced by the pinned commit:
     ```json
     { "type": "git", "url": "https://github.com/JHPG/cosmic-ext-applet-flowmodoro.git", "commit": "<sha>" }
     ```
4. Test with `just build io.github.jhpg.cosmic-ext-applet-flowmodoro` in that repository and open
   a pull request.
