#!/usr/bin/env nix-shell
#!nix-shell -i bash -p bc

{
    cd nixpkgs
    rev=$(git rev-parse HEAD)
    cd ..

    cp ./reproducibility-log "./public/reproducibility-log-$rev"

    reproducible=$(cat ./reproducibility-log | grep '^reproducible' | wc -l)
    total=$(cat ./reproducibility-log | wc -l)
    percent=$(printf "%.3f" "$(echo "($reproducible / $total) * 100" | bc -l)")

    cat <<EOF
<html>
<head>
<title>Is NixOS Reproducible?</title>
<meta name="description" content="nixos-unstable's iso_minimal.x86_64-linux build is $percent% reproducible!" />

<!-- Twitter Card data -->
<meta name="twitter:card" value="summary">

<!-- Open Graph data -->
<meta property="og:title" content="Is NixOS Reproducible?" />
<meta property="og:type" content="article" />
<meta property="og:url" content="https://r13y.com/" />
<meta property="og:image" content="https://nixos.org/logo/nixos-logo-only-hires.png" />
<meta property="og:description" content="nixos-unstable's iso_minimal.x86_64-linux build is $percent% reproducible!" />
</head>
<body>
<h1>Is NixOS Reproducible?</h1>
<h2>Currently tracking: <code>nixos-unstable</code>'s
    <code>iso_minimal</code> job for <code>x86_64-linux</code>.</h2>
<p>Build via:</p>
<pre>
git clone https://github.com/nixos/nixpkgs.git
cd nixpkgs
git checkout $rev
nix-build ./nixos/release-combined.nix -A nixos.iso_minimal.x86_64-linux
</pre>

<h1 style="color: green">$reproducible / $total ($percent%) are reproducible!</h1>
<hr>
<h3>unreproduced paths</h3>
<ul>
EOF

    for drv in $(cat ./reproducibility-log | awk '$1 == "unreproducible" { print $2; }'); do
        cat <<EOF
<li><a href="./diff/$(basename "$drv").html">(diffoscope)</a> <a href=".$drv">(drv)</a> <code>$drv</code></li>
EOF
    done

    cat <<EOF
</ul>
<p><a href="./reproducibility-log-$rev">full list of build results</a></p>
<hr />
<h3 id="test-circumstance">How are these tested?</h3>
<p>Each build is run twice, at different times, on different hardware
running different kernels.</p>

<h3 id="result-confidence">How confident can we be in the results?</h3>

<p>Fairly. We don't currently inject randomness at the filesystem
layer, but many of the reproducibility issues are being exercised
already. It isn't possible to <em>guarantee</em> a package is
reproducible, just like it isn't possible to prove software is
bug-free. It is possible there is nondeterminism in a package source,
waiting for the some specific circumstance.</p>

<p>This is why we run these tests: to track how we are doing over
time, to submit bug fixes for nondeterminism when we find them.</p>

<h3 id="next-steps">How can we do better?</h3>

<p>There are further steps we could take. For example, the next likely
step is using
<a href="https://salsa.debian.org/reproducible-builds/disorderfs">disorderfs</a>
which injects additional nondeterminism by reordering directory entries.
</p>

<hr />

<small>Generated at $(TZ=UTC date) from
<a href="https://github.com/grahamc/r13y.com">https://github.com/grahamc/r13y.com</a></small>
<center><img style="max-width: 100px" src="https://nixos.org/logo/nixos-logo-only-hires.png" /></center>
</body></html>
EOF
}
