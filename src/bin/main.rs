use log::debug;

use itertools::Itertools;
use structopt::StructOpt;

use r13y::{
    check::check,
    messages::{Attr, BuildRequest, BuildRequestV1, Subset},
    report::report,
};

#[derive(StructOpt, Debug)]
struct Opt {
    #[structopt(flatten)]
    nixpkgs: Nixpkgs,

    #[structopt(long = "result-url")]
    result_url: Option<String>,

    #[structopt(subcommand)]
    mode: Mode,

    #[structopt(long = "max-cores", default_value = "3")]
    maximum_cores: u16,
    #[structopt(long = "max-cores-per-job", default_value = "1")]
    maximum_cores_per_job: u16,

    /// Which subsets of nixpkgs to test.
    /// Format: `subset:attr.path | subset`.
    /// subset can be either of "nixpkgs" or "nixos",
    /// attr.path is a dot-delimited attribute path into the preceding subset.
    #[structopt(short = "s", long = "subset", parse(try_from_str = "parse_subset"))]
    subsets: Vec<(Subset, Attr)>,
}

#[derive(StructOpt, Debug)]
struct Nixpkgs {
    /// Nixpkgs revision to use, e.g. 70503758fb4b37107953dfb03ad7c0cf36ad0435
    #[structopt(long = "rev")]
    rev: String,
    /// SHA-256 hashsum of tarball of the given Nixpkgs revision,
    /// e.g. 15g8xckhzpp84p6gv526hb6c1r286qvn8i14w8msw6172jy3kj3c
    #[structopt(long = "sha256")]
    sha256: String,
}

#[derive(StructOpt, Debug)]
enum Mode {
    #[structopt(name = "check")]
    Check,
    #[structopt(name = "report")]
    Report,
}

fn parse_subset(s: &str) -> Result<(Subset, Attr), &'static str> {
    let mut comp = s.split(':');

    let subset = match comp.next() {
        Some("nixpkgs") => Subset::Nixpkgs,
        Some("nixos") => Subset::NixOSReleaseCombined,
        Some(_) => return Err("unknown subset specifier"),
        None => return Err("no subset specifier"),
    };

    let attr_path = if let Some(attrs) = comp.next() {
        attrs.split('.').map(str::to_owned).collect()
    } else {
        Vec::new()
    };

    Ok((subset, attr_path))
}

fn main() {
    env_logger::init();
    let opt = Opt::from_args();

    debug!("Using options: {:#?}", opt);

    let subsets = opt.subsets
        .into_iter().into_group_map().into_iter()
        .map(|(subset, group)| {
            let attrs = if group.iter().any(<Vec<String>>::is_empty) {
                None
            } else {
                Some(group)
            };
            (subset, attrs)
        })
        .collect();

    let instruction = BuildRequest::V1(BuildRequestV1 {
        nixpkgs_revision: opt.nixpkgs.rev,
        nixpkgs_sha256sum: opt.nixpkgs.sha256,
        result_url: opt.result_url.unwrap_or_else(|| String::from("bogus")),
        subsets,
    });

    debug!("Using instruction: {:#?}", instruction);

    match opt.mode {
        Mode::Check => check(instruction, opt.maximum_cores, opt.maximum_cores_per_job),
        Mode::Report => report(instruction),
    }
}
