use log::{debug, info, warn};

mod workqueue;
use workqueue::WorkQueue;

use crate::{
    cas::ContentAddressedStorage,
    derivation::Derivation,
    eval::{eval, JobInstantiation},
    messages::{BuildRequest, BuildResponseV1, BuildStatus, Hashes},
    store::Store,
};

use std::{
    fs::{self, File},
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc::channel,
    thread,
};

enum MoreToDo {
    RetryLonger,
    CaptureCheckDir
}

fn check_reproducibility(thread_id: u16, gc_root_a: &PathBuf, drv: &PathBuf, cores: u16, timeout: Option<usize>) -> Result<BuildStatus,MoreToDo> {
    let first_build = Command::new("nix-store")
        .arg("--add-root")
        .arg(&gc_root_a)
        .arg("--indirect")
        .arg("--realise")
        .arg(&drv)
        .arg("--cores")
        .arg(format!("{}", cores))
        .stdin(Stdio::null())
        .output()
        .expect("failed to execute process");

    debug!(
        "First build of {:?} exited with {:?}",
        &drv,
        first_build.status.code()
    );

    if !first_build.status.success() {
        info!(
            "(thread-{}) First build of {:?} failed. Result:\n#{:#?}",
            thread_id, &drv, first_build
        );

        return Ok(BuildStatus::FirstFailed);
    }

    debug!(
        "(thread-{}) Performing --check build: {:#?}",
        thread_id, drv
    );
    let second_build = Command::new("nix-store")
        .arg("--realise")
        .arg(&drv)
        .arg("--cores")
        .arg(format!("{}", cores))
        .arg("--timeout")
        .arg(format!("{}", timeout.unwrap_or(0)))
        .arg("--check")
        .arg("--keep-failed")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to execute process")
        .wait()
        .expect("failed to wait");
    debug!(
        "Second build of {:?} exited with {:?}",
        &drv,
        second_build.code()
    );

    if second_build.success() {
        info!("(thread-{}) Reproducible: {:?}", thread_id, drv);
        return Ok(BuildStatus::Reproducible);
    } else if second_build.code() == Some(101) {
        info!("(thread-{}) Needs more time: {:?}", thread_id, drv);
        return Err(MoreToDo::RetryLonger);
    } else {
        info!("(thread-{}) Unreproducible: {:?}", thread_id, drv);
        return Err(MoreToDo::CaptureCheckDir);
    }
}

fn calc(drv: &PathBuf, store: &Store, gc_root_check: &PathBuf, cas: &ContentAddressedStorage) -> BuildStatus {
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
        let check_path = path.with_file_name(check_name);

        debug!("Looking for {:?}", check_path);

        if check_path.exists() {
            debug!("Found {:?}", check_path);
            let checked =
                store.add_path(&check_path, &gc_root_check).unwrap();

            let (path_stream, mut path_wait) =
                store.export_nar(&path).unwrap();
            let (checked_stream, mut checked_wait) =
                store.export_nar(&checked).unwrap();

            hashes.insert(
                output.to_string(),
                (
                    cas.from_read(path_stream).unwrap().into(),
                    cas.from_read(checked_stream).unwrap().into(),
                ),
            );

            println!("{:#?}", hashes);

            path_wait.wait().unwrap();
            checked_wait.wait().unwrap();
        } else {
            debug!("Did not find {:?}", check_path);
        }
    }

    if hashes.is_empty() {
        BuildStatus::SecondFailed
    } else {
        BuildStatus::Unreproducible(hashes)
    }
}

pub fn check(instruction: BuildRequest, maximum_cores: u16, maximum_cores_per_job: u16) {
    let job = match instruction {
        BuildRequest::V1(ref req) => req.clone(),
    };

    let (result_tx, result_rx) = channel();
    let tmpdir = PathBuf::from("./tmp/");

    let JobInstantiation {
        mut to_build, mut results, skip_list, ..
    } = eval(instruction.clone());

    // Remove builds that have succeeded before, by holding onto everything not on the skip list
    to_build.retain(|drv| !skip_list.contains(drv));
    let to_build_len = to_build.len();

    let mut queue: WorkQueue = WorkQueue::new(to_build.into_iter().collect());

    let cas = ContentAddressedStorage::new(tmpdir.clone());

    // In the future, only give 1 core to jobs which don't allow
    // parallel builds
    let timeout_seconds = None;
    let slow_queue: WorkQueue = WorkQueue::new(vec![]);
    let thread_count = maximum_cores / maximum_cores_per_job;
    info!("Starting {} threads", thread_count);
    let threads: Vec<thread::JoinHandle<()>> = ((0 + 1)..=thread_count)
        .map(|thread_id| {
            info!("Starting thread {}", thread_id);

            let result_tx = result_tx.clone();
            let queue = queue.clone();
            let mut slow_queue = slow_queue.clone();
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

                    for drv in queue {
                        info!("(thread-{}) Checking: {:#?}", thread_id, drv);
                        match check_reproducibility(thread_id, &gc_root_a, &drv, maximum_cores_per_job, timeout_seconds) {
                            Ok(status) => {
                                result_tx.send(BuildResponseV1 {
                                    request: request.clone(),
                                    drv: drv.to_str().unwrap().to_string(),
                                    status: status,
                                }).unwrap();
                            }
                            Err(MoreToDo::RetryLonger) => {
                                slow_queue.push(drv);
                            },
                            Err(MoreToDo::CaptureCheckDir) => {
                                result_tx.send(BuildResponseV1 {
                                    request: request.clone(),
                                    drv: drv.to_str().unwrap().to_string(),
                                    status: calc(&drv, &store, &gc_root_check, &cas),
                                }).unwrap();
                            },
                        }
                    }

                    debug!("no more work, shutting down {}", thread_id);
                })
                .unwrap()
        })
        .collect();
    drop(result_tx);

    let mut i = 0;
    let mut total = 0;

    let mut requeues: Vec<String> = vec![];

    for response in result_rx.iter() {
        i += 1;
        total += 1;
        if i == 10 {
            i = 0;
            debug!("Writing out interim state to the reproducibility log");
            let mut log_file = File::create(format!(
                "reproducibility-log-{}.json",
                &job.nixpkgs_revision
            ))
            .unwrap();
            log_file
                .write_all(serde_json::to_string(&results).unwrap().as_bytes())
                .unwrap();
        }

        if response.status == BuildStatus::FirstFailed {
            if requeues.contains(&response.drv) {
                warn!("FirstFailed, retried, failed again: {:#?}", response);
                results.push(response);
                if requeues.len() > 3 {
                    panic!("Too many builds failed first time around.");
                }
            } else {
                warn!("FirstFailed, requeueing {:#?}", response);
                requeues.push(response.drv.clone());
                queue.push(PathBuf::from(response.drv));

                total -= 1;
            }
        } else {
            results.push(response);
            println!("{} / {}", total, to_build_len);
        }
    }

    for thread in threads {
        thread.join().unwrap();
    }

    let mut log_file = File::create(format!(
        "reproducibility-log-{}.json",
        &job.nixpkgs_revision
    ))
    .unwrap();
    log_file
        .write_all(serde_json::to_string(&results).unwrap().as_bytes())
        .unwrap();
}
