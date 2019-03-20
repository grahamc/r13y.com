extern crate r13y;
extern crate serde;
#[macro_use] extern crate log;
extern crate env_logger;
extern crate serde_json;
extern crate rand;
extern crate sha2;
extern crate digest;
extern crate tempdir;
use std::thread;
use std::env;
use std::fs;
use std::fs::File;
use std::process::{Command, Stdio};
use std::path::{Path, PathBuf};
use std::collections::HashSet;
use std::io::{Write, BufRead};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::channel;
use r13y::messages::{
    BuildRequest,
    BuildRequestV1,
    BuildResponseV1,
    BuildStatus,
    Subset,
    Hashes,
    Attr,
};
use r13y::derivation::Derivation;
use r13y::store::Store;
use r13y::contentaddressedstorage::ContentAddressedStorage;
use rand::seq::SliceRandom;

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


    let mut results: Vec<BuildResponseV1>;
    let mut skip_list: HashSet<String> = HashSet::new();

    if let Ok(log_file) = File::open(format!("reproducibility-log-{}.json", &job.nixpkgs_revision)) {
        results = serde_json::from_reader(log_file).unwrap();

        for elem in results.iter() {
            skip_list.insert(elem.drv.clone());
        }
    } else {
        results = vec![];
    };

    let (result_tx, result_rx) = channel();


    let tmpdir = PathBuf::from("./tmp/");


    let mut to_build: HashSet<PathBuf> = HashSet::new();

    for (subset, attrs) in job.subsets.into_iter() {
        let drv = {
            let mut drv = tmpdir.clone();
            drv.push("result.drv");
            drv
        };
        let path: &'static Path = (&subset).into();
        let attrs: Vec<Attr> = attrs.unwrap_or(vec![]);

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
                    if ! skip_list.contains(&line) {
                        to_build.insert(line.into());
                    }
                }
            }
        }
    }


    let to_build_len = to_build.len();
    let queue: Arc<Mutex<Vec<PathBuf>>> = Arc::new(Mutex::new(to_build.into_iter().collect()));
    queue.lock().unwrap().shuffle(&mut rand::thread_rng());

    let cas = ContentAddressedStorage::new(tmpdir.clone());

    let maximum_cores = 8;
    let maximum_cores_per_job = 2;

    // In the future, only give 1 core to jobs which don't allow
    // parallel builds
    let thread_count = maximum_cores / maximum_cores_per_job;
    info!("Starting {} threads", thread_count);
    let threads: Vec<thread::JoinHandle<()>> = ((0 + 1)..=thread_count).map(|thread_id| {
        info!("Starting thread {}", thread_id);

        let result_tx = result_tx.clone();
        let queue = queue.clone();
        let mut tmpdir = tmpdir.clone();
        tmpdir.push(format!("thread-{}", thread_id));

        let request = instruction.clone();
        fs::create_dir_all(&tmpdir).unwrap();

        let mut gc_root_a = tmpdir.clone();
        gc_root_a.push("buildA");

        let mut gc_root_check = tmpdir.clone();
        gc_root_check.push("check");
        let cas = cas.clone();

        thread::Builder::new()
            .name(format!("builder-{}", thread_id))
            .spawn(move || {
                let store = Store::new();

                loop {
                    let drv = {
                        let mut queue_unlocked = queue.lock().unwrap();
                        let job = queue_unlocked.pop();
                        drop(queue_unlocked);

                        if let Some(job) = job {
                            job
                        } else {
                            debug!("no more work, shutting down {}", thread_id);
                            return;
                        }
                    };

                    debug!("(thread-{}) Building drv: {:#?}", thread_id, drv);

                    let first_build = Command::new("nix-store")
                        .arg("--add-root").arg(&gc_root_a).arg("--indirect")
                        .arg("--realise").arg(&drv)
                        .arg("--cores").arg(format!("{}", maximum_cores_per_job))
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn()
                        .expect("failed to execute process")
                        .wait()
                        .expect("failed to wait");
                    debug!("First build of {:?} exited with {:?}",
                          &drv, first_build.code()
                    );
                    if ! first_build.success() {
                        info!("First build of {:?} failed", &drv);
                        result_tx.send(BuildResponseV1 {
                            request: request.clone(),
                            drv: drv.to_str().unwrap().to_string(),
                            status: BuildStatus::FirstFailed
                        }).unwrap();
                        continue
                    }

                    debug!("(thread-{}) Performing --check build: {:#?}", thread_id, drv);
                    let second_build = Command::new("nix-store")
                        .arg("--realise").arg(&drv)
                        .arg("--cores").arg(format!("{}", maximum_cores_per_job))
                        .arg("--check")
                        .arg("--keep-failed")
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn()
                        .expect("failed to execute process")
                        .wait()
                        .expect("failed to wait");
                    debug!("Second build of {:?} exited with {:?}",
                          &drv, second_build.code()
                    );

                    if second_build.success() {
                        info!("Reproducible: {:?}", &drv);
                        result_tx.send(BuildResponseV1 {
                            request: request.clone(),
                            drv: drv.to_str().unwrap().to_string(),
                            status: BuildStatus::Reproducible
                        }).unwrap();
                    } else {
                        info!("Unreproducible: {:?}", &drv);
                        let parsed_drv = Derivation::parse(&drv).unwrap();

                        // For each output, look for a .check directory.
                        // If we find one, we want to:
                        //
                        // 1. add it to the store right away -- .check directories
                        //    aren't actually store paths and cannot be saved from
                        //    being garbage collected
                        //
                        // 2. create a GC root for what we just added to the store
                        //    see: https://github.com/NixOS/nix/issues/2676
                        //
                        // 3. create a NAR for the .check store path
                        //
                        // 4. create a NAR for the output store path
                        //
                        // 5. hash the two NARs
                        //
                        // 6. return a build result with the two hashes
                        let mut hashes: Hashes = Hashes::new();

                        for (output, path) in parsed_drv.outputs().iter() {
                            // with_extension, naively, will replace foo-1.2.3 with foo-1.2.check
                            let mut check_name = path
                                .file_name()
                                .expect("should have a file name")
                                .to_owned();
                            check_name.push(".check");
                            let mut check_path = path.with_file_name(check_name);

                            debug!("Looking for {:?}", check_path);

                            if check_path.exists() {
                                debug!("Found {:?}", check_path);
                                let checked = store.add_path(&check_path, &gc_root_check).unwrap();

                                let (path_stream, mut path_wait) = store.export_nar(&path).unwrap();
                                let (checked_stream, mut checked_wait) = store.export_nar(&checked).unwrap();

                                hashes.insert(output.to_string(), (
                                    cas.from_read(path_stream).unwrap().into(),
                                    cas.from_read(checked_stream).unwrap().into(),
                                ));

                                println!("{:#?}", hashes);

                                path_wait.wait().unwrap();
                                checked_wait.wait().unwrap();
                            } else {
                                debug!("Did not find {:?}", check_path);
                            }
                        }

                        if hashes.is_empty() {
                            result_tx.send(BuildResponseV1 {
                                request: request.clone(),
                                drv: drv.to_str().unwrap().to_string(),
                                status: BuildStatus::SecondFailed,
                            }).unwrap();
                        } else {
                            result_tx.send(BuildResponseV1 {
                                request: request.clone(),
                                drv: drv.to_str().unwrap().to_string(),
                                status: BuildStatus::Unreproducible(hashes)
                            }).unwrap();
                        }
                    }


                }
            })
            .unwrap()
    }).collect();
    drop(result_tx);

    let mut i = 0;
    let mut total = 0;

    for response in result_rx.iter() {
        i += 1;
        total += 1;
        if i == 10 {
            i = 0;
            debug!("Writing out interim state to the reproducibility log");
            let mut log_file = File::create(format!("reproducibility-log-{}.json", &job.nixpkgs_revision)).unwrap();
            log_file.write_all(
                serde_json::to_string(&results).unwrap().as_bytes(),
            ).unwrap();
        }

        /*
        let status = match response.status {
            BuildStatus::FirstFailed => "first-failed",
            BuildStatus::SecondFailed => "second-failed",
            BuildStatus::Reproducible => "reproducible",
            BuildStatus::Unreproducible(_) => "unreproducible",
        };
        */

        results.push(response);
        println!("{} / {}", total, to_build_len);
    }

    for thread in threads {
        thread.join().unwrap();
    }

    let mut log_file = File::create(format!("reproducibility-log-{}.json", &job.nixpkgs_revision)).unwrap();
    log_file.write_all(
        serde_json::to_string(&results).unwrap().as_bytes(),
    ).unwrap();

}
