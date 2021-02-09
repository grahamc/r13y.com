#!/usr/bin/env nix-shell
#!nix-shell -i bash -p awscli -I nixpkgs=channel:nixos-unstable-small

buildkite-agent artifact download report.tar.xz ./
tar -xf ./report.tar.xz

aws s3 cp ./report s3://r13y-com/ --recursive --acl public-read
