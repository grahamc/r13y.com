use log::{debug, info};

use crate::messages::{Attr, BuildRequest, BuildResponseV1, BuildStatus};

use std::{
    collections::HashSet,
    fs::File,
    io::BufRead,
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn log_command_output(output: Output) {
    for line in output.stderr.lines() {
        info!("stderr: {:?}", line)
    }

    for line in output.stdout.lines() {
        debug!("stdout: {:?}", line)
    }
}

pub fn load_r13y_log(rev: &str) -> Vec<BuildResponseV1> {
    if let Ok(log_file) = File::open(format!("reproducibility-log-{}.json", rev)) {
        serde_json::from_reader(log_file).expect("Unable to parse r13y log")
    } else {
        Vec::new()
    }
}

pub struct JobInstantiation {
    pub results: Vec<BuildResponseV1>,
    pub to_build: HashSet<PathBuf>,
    pub skip_list: HashSet<PathBuf>
}

pub fn eval(instruction: BuildRequest) -> JobInstantiation {
    let job = match instruction {
        BuildRequest::V1(ref req) => req.clone(),
    };

    let mut results = Vec::new();

    let mut skip_list = HashSet::new();
    let prev_results = load_r13y_log(&job.nixpkgs_revision);
    for elem in prev_results.into_iter() {
        if elem.status == BuildStatus::FirstFailed {
            info!(
                "Ignoring for skiplist as it failed the first time: {:#?}",
                &elem
            );
        } else {
            skip_list.insert(PathBuf::from(&elem.drv));
            results.push(elem);
        }
    }

    let tmpdir = PathBuf::from("./tmp/");

    let mut to_build: HashSet<PathBuf> = HashSet::new();

    for (subset, attrs) in job.subsets.into_iter() {
        let drv = tmpdir.join("result.drv");
        let path: &Path = (&subset).into();
        let attrs: Vec<Attr> = attrs.unwrap_or_default();

        info!("Evaluating {:?} {:#?}", &subset, &attrs);
        let eval = Command::new("nix-instantiate")
            // .arg("--pure-eval") // See evaluate.nix for why this isn't passed yet
            .arg("-E")
            .arg(include_str!("./evaluate.nix"))
            .arg("--add-root")
            .arg(&drv)
            .arg("--indirect")
            .args(&[
                "--argstr",
                "revision",
                &job.nixpkgs_revision,
                "--argstr",
                "sha256",
                &job.nixpkgs_sha256sum,
                "--argstr",
                "subfile",
                &path.display().to_string(),
                "--argstr",
                "attrsJSON",
                &serde_json::to_string(&attrs).unwrap(),
            ])
            .output()
            .expect("failed to execute process");
        log_command_output(eval);

        let query_requisites = Command::new("nix-store")
            .arg("--query")
            .arg("--requisites")
            .arg(&drv)
            .output()
            .expect("failed to execute process");

        for line in query_requisites.stdout.lines().filter_map(Result::ok) {
            if line.ends_with(".drv") {
                to_build.insert(line.into());
            }
        }
        log_command_output(query_requisites);
    }

    JobInstantiation { to_build, results, skip_list }
}
