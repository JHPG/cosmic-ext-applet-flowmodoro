bin := env_var('HOME') / ".local/bin/cosmic-flowmodoro"
desktop := env_var('HOME') / ".local/share/applications/com.github.cosmic-flowmodoro.desktop"

build:
    cargo build --release

test:
    cargo test

install: build
    install -Dm755 target/release/cosmic-flowmodoro {{bin}}
    sed 's|^Exec=.*|Exec={{bin}}|' com.github.cosmic-flowmodoro.desktop | install -Dm644 /dev/stdin {{desktop}}

uninstall:
    rm -f {{bin}} {{desktop}}
