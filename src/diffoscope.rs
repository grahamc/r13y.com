use std::io;
use std::fs;
use std::fs::{File, create_dir_all};
use std::process::{Command, Stdio};
use std::path::{Path, PathBuf};
use tempdir::TempDir;
use contentaddressedstorage::ContentAddressedStorage;

#[derive(Clone)]
pub struct Diffoscope {
    storage: ContentAddressedStorage,
}

impl Diffoscope {
    pub fn new(storage: ContentAddressedStorage) -> Diffoscope {
        Diffoscope {
            storage,
        }
    }

    pub fn nars(&self, name: &str, path_a: &Path, path_b: &Path) -> Result<PathBuf, io::Error> {
        assert!(!name.contains("/"));
        let tempdir = TempDir::new("diffoscope-scratch").unwrap();
        let relative_a = PathBuf::from(name).join("A");
        let relative_b = PathBuf::from(name).join("B");

        let dest_a = tempdir.path().join(&relative_a);
        create_dir_all(&dest_a.parent().unwrap()).unwrap();
        let dest_b = tempdir.path().join(&relative_b);

        {
            warn!("Opening {:?}", path_a);
            let mut open_a = File::open(path_a)?;
            warn!("Opened {:?}", path_a);

            let mut load_a = Command::new("nix-store")
                .arg("--restore")
                .arg(&dest_a)
                .stdin(Stdio::piped())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()
                .expect("failed to execute process");
            io::copy(&mut open_a, &mut load_a.stdin.take().unwrap()).unwrap();
            load_a.wait().unwrap();
            fix_time(&dest_a)?;
        }

        {
            warn!("Opening {:?}", path_b);
            let mut open_b = File::open(path_b)?;
            warn!("Opened {:?}", path_b);
            let mut load_b = Command::new("nix-store")
                .arg("--restore")
                .arg(&dest_b)
                .stdin(Stdio::piped())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()
                .expect("failed to execute process");
            io::copy(&mut open_b, &mut load_b.stdin.take().unwrap()).unwrap();
            load_b.wait().unwrap();
            fix_time(&dest_b)?;
        }

        println!("{:?}", dest_a.exists());
        println!("{:?}", dest_b.exists());

        let mut diff = Command::new("diffoscope")
            .arg("--html")
            .arg("-")
            .current_dir(&tempdir)
            .arg(&relative_a)
            .arg(&relative_b)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("failed to execute process");

        let result = self.storage.from_read(
            diff.stdout.take().unwrap()
        ).unwrap().as_path_buf();

        if diff.wait()?.success() {
            drop(tempdir);
            Ok(result)
        } else {
            panic!("Diffoscope exited non-zero");
        }
    }
}


fn fix_time(path: &Path) -> io::Result<()> {
    let chtime = Command::new("touch")
        .arg("--date")
        .arg("@1")
        .arg("--no-dereference")
        .arg(&path)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .expect("Failed to execute process");

    if !chtime.success() {
        panic!("Failed to touch {:?}", path);
    }

    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            fix_time(&entry.path())?;
        }
    }
    Ok(())
}
