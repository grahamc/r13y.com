extern crate r13y;
extern crate serde;
#[macro_use] extern crate log;
extern crate env_logger;
extern crate serde_json;
extern crate chrono;
extern crate rand;
extern crate sha2;
extern crate digest;
extern crate tempdir;
use std::env;
use std::fs;
use std::fs::File;
use std::process::Command;
use std::path::{Path, PathBuf};
use std::collections::HashSet;
use std::io::{Write, BufRead};
use r13y::messages::{
    BuildRequest,
    BuildRequestV1,
    BuildResponseV1,
    BuildStatus,
    Subset,
    Attr,
};
use r13y::derivation::Derivation;
use r13y::contentaddressedstorage::ContentAddressedStorage;
use r13y::diffoscope::Diffoscope;
use chrono::Utc;

fn main() {
    env_logger::init();

    let instruction = BuildRequest::V1(
        BuildRequestV1 {
            nixpkgs_revision: env::args().nth(1).unwrap(),
            nixpkgs_sha256sum: env::args().nth(2).unwrap(),
            result_url: "bogus".into(),
            subsets: vec![
                (
                    Subset::NixOSReleaseCombined,
                    Some(vec![
                        vec!["nixos".into(),
                             "iso_minimal".into(),
                             "x86_64-linux".into()]
                    ])
                )
            ].into_iter().collect()
        }
    );


    let job = match instruction {
        BuildRequest::V1(ref req) => req.clone(),
    };
    let tmpdir = PathBuf::from("./tmp/");
    let results: Vec<BuildResponseV1> = serde_json::from_reader(File::open(
        format!("reproducibility-log-{}.json", job.nixpkgs_revision)
    ).unwrap()).unwrap();


    let mut to_build: HashSet<String> = HashSet::new();

    for (subset, attrs) in job.subsets.iter() {
        let drv = {
            let mut drv = tmpdir.clone();
            drv.push("result.drv");
            drv
        };
        let path: PathBuf = Into::<&'static Path>::into(subset).to_path_buf();
        let attrs: Vec<Attr> = attrs.clone().unwrap_or(vec![]);

        info!("Evaluating {:?} {:#?}", &subset, &attrs);
        let eval = Command::new("nix-instantiate")
             // .arg("--pure-eval") // See evaluate.nix for why this isn't passed yet
            .arg("-E")
            .arg(include_str!("./evaluate.nix"))
            .arg("--add-root").arg(&drv).arg("--indirect")
            .args(&[
                "--argstr", "revision",  &job.nixpkgs_revision,
                "--argstr", "sha256",    &job.nixpkgs_sha256sum,
                "--argstr", "subfile",   &format!("{}", path.display()),
                "--argstr", "attrsJSON", &serde_json::to_string(&attrs).unwrap(),
            ])
            .output()
            .expect("failed to execute process");

        for line in eval.stderr.lines() {
            info!("stderr: {:?}", line)
        }
        for line in eval.stdout.lines() {
            info!("stdout: {:?}", line)
        }

        let query_requisites = Command::new("nix-store")
            .arg("--query")
            .arg("--requisites")
            .arg(&drv)
            .output()
            .expect("failed to execute process");
        for line in query_requisites.stderr.lines() {
            info!("stderr: {:?}", line);
        }
        for line in query_requisites.stdout.lines() {
            info!("stdout: {:?}", &line);
            if let Ok(line) = line {
                if line.ends_with(".drv") {
                    to_build.insert(line.into());
                }
            }
        }
    }


    let report_dir = PathBuf::from("./report/");
    std::fs::create_dir_all(&report_dir).unwrap();
    let diff_dir = PathBuf::from("./report/diff");
    std::fs::create_dir_all(&diff_dir).unwrap();
    let mut html = File::create(report_dir.join("index.html")).unwrap();


    let read_cas = ContentAddressedStorage::new(tmpdir.clone());
    let write_cas = ContentAddressedStorage::new(report_dir.clone().join("cas"));
    let diffoscope = Diffoscope::new(write_cas.clone());
    let mut total = 0;
    let mut reproducible = 0;
    let mut unreproducible_list: Vec<String> = vec![];
    let mut unchecked = 0;
    for response in results
        .into_iter()
        .filter(|response|
                (match response.request {
                    BuildRequest::V1(ref req) => req.nixpkgs_revision == job.nixpkgs_revision,
                }) && to_build.contains(&response.drv)
        )
    {
        total += 1;
        match response.status {
            BuildStatus::Reproducible => { reproducible += 1; },
            BuildStatus::FirstFailed => { unchecked += 1; },
            BuildStatus::SecondFailed => { unchecked += 1; },
            BuildStatus::Unreproducible(hashes) => {
                let parsed_drv = Derivation::parse(&Path::new(&response.drv)).unwrap();

                unreproducible_list.push(format!(
                    "<li><code>{}</code><ul>",
                    response.drv
                ));
                for (output, (hash_a, hash_b)) in hashes.iter() {

                    if let Some(output_path) = parsed_drv.outputs().get(output) {
                        let dest_name = format!("{}-{}.html", hash_a, hash_b);
                        let dest = diff_dir.join(&dest_name);

                        if dest.exists() {
                            // ok
                        } else {
                            println!("Diffing {}'s {}: {} vs {}",
                                     response.drv, output, hash_a, hash_b
                            );

                            let cas_a = read_cas.str_to_id(hash_a).unwrap();
                            let cas_b = read_cas.str_to_id(hash_b).unwrap();
                            let savedto = diffoscope.nars(
                                &output_path.file_name().unwrap().to_string_lossy(),
                                &cas_a.as_path_buf(),
                                &cas_b.as_path_buf()
                            ).unwrap();
                            println!("saved to: {}", savedto.display());
                            fs::copy(
                                savedto,
                                dest
                            ).unwrap();
                        }
                        unreproducible_list.push(format!(
                            "<li><a href=\"./diff/{}\">(diffoscope)</a> {}</li>",
                            dest_name,
                            output
                        ));

                    } else {
                        println!("Diffing {} but no output named {}",
                                 response.drv, output
                        );
                        // <li><a href="./diff/59nzffg69nprgg2zp8b36rqwha8vxzjk-perl-5.28.1.drv.html">(diffoscope)</a> <a href="./nix/store/59nzffg69nprgg2zp8b36rqwha8vxzjk-perl-5.28.1.drv">(drv)</a> <code>/nix/store/59nzffg69nprgg2zp8b36rqwha8vxzjk-perl-5.28.1.drv</code></li>
                    }
                }
                unreproducible_list.push(format!(
                    "</ul></li>"
                ));

                println!("{:#?}", hashes);
            },
        }
    }

    html.write_all(format!(
        include_str!("./template.html"),
        reproduced = reproducible,
        unchecked = unchecked,
        total = total,
        percent = format!("{:.*}%",  2, 100.0 * (reproducible as f64 / total as f64)),
        revision = job.nixpkgs_revision,
        now = Utc::now().to_string(),
        unreproduced_list = unreproducible_list.join("\n")

    ).as_bytes()).unwrap();

}
