#!/usr/bin/env nix-shell
#!nix-shell -i bash -p jq diffoscope nix git findutils coreutils --pure

set -eux


export LANG=en_US.UTF-8
export LOCALE_ARCHIVE=/run/current-system/sw/lib/locale/locale-archive

CORES=$(nproc)
LOGFILE=./reproducibility-log
REPORT_STORE=$(pwd)/public

function update_nixpkgs() (
    if [ ! -d ./nixpkgs ]; then
        git clone https://github.com/nixos/nixpkgs-channels.git ./nixpkgs
    fi


    cd nixpkgs
    # git fetch origin
    #git checkout origin/nixos-unstable
)

function nix_store_path_requisite_drvs() {
    nix-store --query --requisites "$1" | grep '\.drv$'
}

function find_iso_minimal_drv_x86_64_linux() (
    nix-instantiate \
        ./nixpkgs/nixos/release-combined.nix \
        -A nixos.iso_minimal.x86_64-linux \
        --add-root ./result.drv --indirect
)

function drv_outputs() {
    nix show-derivation "$1" \
        | jq -r '. | map(.outputs) | .[0] | map(.path) | .[]'
}

function drv_name() (
    echo "$1" | tail -c+45
)

function log() {
    status="$1"
    drv="$2"

    printf "%s\t%s\n" "$status" "$drv" | tee -a "$LOGFILE"
}

function have_checked_before_old() {
    grep -q "$1" "$LOGFILE.b"
}

function have_checked_before() {
    grep -q "$1" "$LOGFILE"
}

function main() {
    update_nixpkgs

    top_level_drv=$(find_iso_minimal_drv_x86_64_linux)
    printf "ISO Drv: %s\n" "$top_level_drv"

    mkdir -p "$REPORT_STORE/diff"

    attempted=0
    total=$(nix_store_path_requisite_drvs "$top_level_drv" | wc -l)

    for drv in $(nix_store_path_requisite_drvs "$top_level_drv"); do
        grep "$drv" reproducibily-log-c >> reproducibility-log-d
    done
}

main
