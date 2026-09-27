name := 'cosmic-applet-lowkey-workspaces'
appid := 'dev.daenu.CosmicAppletLowkeyWorkspaces'

prefix := env('HOME') / '.local'
bin-dst := prefix / 'bin' / name
desktop-dst := prefix / 'share' / 'applications' / appid + '.desktop'
icon-dst := prefix / 'share' / 'icons' / 'hicolor' / 'scalable' / 'apps' / appid + '.svg'

default: build-release

build-release *args:
    cargo build --release {{args}}

run:
    cargo run --release

install: build-release
    install -Dm0755 target/release/{{name}} {{bin-dst}}
    install -Dm0644 data/{{appid}}.desktop {{desktop-dst}}
    install -Dm0644 data/icons/scalable/apps/{{appid}}.svg {{icon-dst}}
    sed -i 's|^Exec=.*|Exec={{bin-dst}}|' {{desktop-dst}}

uninstall:
    rm -f {{bin-dst}} {{desktop-dst}} {{icon-dst}}
