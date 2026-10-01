id := "io.github.jhpg.cosmic-ext-applet-flowmodoro"
name := "cosmic-ext-applet-flowmodoro"
share := env_var('HOME') / ".local/share"
bin := env_var('HOME') / ".local/bin" / name

build:
    cargo build --release

test:
    cargo test

# install for the current user (~/.local), no root needed
install: build
    install -Dm755 target/release/{{name}} {{bin}}
    sed 's|^Exec=.*|Exec={{bin}}|' data/{{id}}.desktop | install -Dm644 /dev/stdin {{share}}/applications/{{id}}.desktop
    install -Dm644 data/{{id}}-symbolic.svg {{share}}/icons/hicolor/scalable/apps/{{id}}-symbolic.svg

uninstall:
    rm -f {{bin}} {{share}}/applications/{{id}}.desktop {{share}}/icons/hicolor/scalable/apps/{{id}}-symbolic.svg

# regenerate flatpak/cargo-sources.json after changing Cargo.lock
flatpak-sources:
    curl -sSfo /tmp/flatpak-cargo-generator.py https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py
    uv run --with aiohttp --with tomlkit python /tmp/flatpak-cargo-generator.py Cargo.lock -o flatpak/cargo-sources.json

# build and install the flatpak locally (needs org.flatpak.Builder from Flathub)
flatpak:
    flatpak run org.flatpak.Builder --user --install --force-clean --install-deps-from=flathub flatpak/build flatpak/{{id}}.json

validate:
    appstreamcli validate --no-net data/{{id}}.metainfo.xml
    desktop-file-validate data/{{id}}.desktop
