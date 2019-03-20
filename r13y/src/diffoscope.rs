use std::io;
use std::fs;
use std::fs::{File, rename, create_dir_all};
use std::io::{BufReader, Read, Write, BufWriter};
use std::process::{Command, Stdio};
use std::path::{Path, PathBuf};
use sha2::Sha256;
use sha2::Digest;
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

    pub fn nars(&self, name: &str, pathA: &Path, pathB: &Path) -> Result<PathBuf, io::Error> {
        assert!(!name.contains("/"));
        let tempdir = TempDir::new("diffoscope-scratch").unwrap();
        let relativeA = PathBuf::from(name).join("A");
        let relativeB = PathBuf::from(name).join("B");

        let destA = tempdir.path().join(&relativeA);
        create_dir_all(&destA.parent().unwrap()).unwrap();
        let destB = tempdir.path().join(&relativeB);

        {
            warn!("Opening {:?}", pathA);
            let mut openA = File::open(pathA)?;
            warn!("Opened {:?}", pathA);

            let mut loadA = Command::new("nix-store")
                .arg("--restore")
                .arg(&destA)
                .stdin(Stdio::piped())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()
                .expect("failed to execute process");
            io::copy(&mut openA, &mut loadA.stdin.take().unwrap()).unwrap();
            loadA.wait().unwrap();
            fix_time(&destA)?;
        }

        {
            warn!("Opening {:?}", pathB);
            let mut openB = File::open(pathB)?;
            warn!("Opened {:?}", pathB);
            let mut loadB = Command::new("nix-store")
                .arg("--restore")
                .arg(&destB)
                .stdin(Stdio::piped())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()
                .expect("failed to execute process");
            io::copy(&mut openB, &mut loadB.stdin.take().unwrap()).unwrap();
            loadB.wait().unwrap();
            fix_time(&destB)?;
        }

        println!("{:?}", destA.exists());
        println!("{:?}", destB.exists());

        let mut diff = Command::new("diffoscope")
            .arg("--html")
            .arg("-")
            .current_dir(&tempdir)
            .arg(&relativeA)
            .arg(&relativeB)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("failed to execute process");

        let result = self.storage.from_read(
            diff.stdout.take().unwrap()
        ).unwrap().as_path_buf();
        diff.wait().unwrap();
        drop(tempdir);
        Ok(result)
    }
}


fn fix_time(dir: &Path) -> io::Result<()> {
    let mut chtime = Command::new("touch")
        .arg("--date")
        .arg("@1")
        .arg("--no-dereference")
        .arg(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .expect("failed to execute process");

    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            fix_time(&entry.path());
        }
    }
    Ok(())
}
