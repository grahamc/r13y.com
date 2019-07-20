# What is this?

This repo is the tooling and automation to generate https://r13y.com,
a website which tracks how reproducible the NixOS package builds are.

See https://reproducible-builds.org/ for information about why
reproducible builds matter, other projects involved in the effort,
and also a collection of tools and other information about
reproducible builds.

# How can I run this?

Right now, this repository builds every date at
https://buildkite.com/grahamc/r13y-dot-com/

If you want to run it yourself, check out `./check.sh`. It will need
minor modifications (the `rsync` line) to complete successfully.

# What might be next for the project?

Check out https://github.com/grahamc/r13y.com/issues/4 for some
discussion about what might be next.
