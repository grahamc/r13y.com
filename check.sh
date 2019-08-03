#!/usr/bin/env nix-shell
#!nix-shell -i bash ./default.nix -I nixpkgs=channel:nixos-19.03

set -eux

#export LANG=en_US.UTF-8
#export LOCALE_ARCHIVE=/run/current-system/sw/lib/locale/locale-archive

function fetch_temps() (
    curl 'https://api.weather.gov/gridpoints/ALY/76,52/forecast/hourly?units=si' \
        | jq -r '.properties.periods | limit(24;.[]) | .temperature | select(. > 26)'
)
function check_temp() {
    if [ $(fetch_temps | wc -l) -gt 5 ]; then
        echo "too hot";
        exit 1
    fi
}

function nixpkgs_rev() (
    curl https://channels.nix.gsc.io/nixos-unstable-small/latest | cut -d' ' -f1
)

function main() {
    check_temp

    export REV=$(nixpkgs_rev)
    export HASH=$(nix-prefetch-url --unpack "https://github.com/NixOS/nixpkgs/archive/${REV}.tar.gz")
    export SUBSET=nixos:nixos.iso_minimal.x86_64-linux
    export RUST_BACKTRACE=1
    (unset RUST_LOG; cargo build)

    cargo run -- \
          --subset "$SUBSET" \
          --rev "$REV" \
          --sha256 "$HASH" \
          check

    cargo run -- \
          --subset "$SUBSET" \
          --rev "$REV" \
          --sha256 "$HASH" \
          report

    rsync -e "ssh -i /etc/r13y-ssh-private" -r ./report/ r13y@r13y.com:/var/lib/nginx/r13y/r13y.com
}

main
