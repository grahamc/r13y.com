#!/usr/bin/env nix-shell
#!nix-shell -i bash -p awscli -I nixpkgs=channel:nixos-unstable-small

buildkite-agent artifact download report-gnome.tar.xz ./
tar -xf ./report-gnome.tar.xz

aws s3 cp ./report-gnome s3://r13y-com/gnome --recursive --acl public-read
