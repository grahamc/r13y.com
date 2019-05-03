use sha2::Digest;
use sha2::Sha256;
use std::fs::{create_dir_all, rename, File};
use std::io;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;
use tempdir::TempDir;

#[derive(Clone)]
pub struct ContentAddressedStorage {
    root: PathBuf,
}

impl ContentAddressedStorage {
    pub fn new(root: PathBuf) -> ContentAddressedStorage {
        ContentAddressedStorage { root }
    }

    pub fn str_to_id(&self, id: &str) -> Option<ID> {
        let path = self.root.join(id);
        if path.exists() {
            Some(ID(id.to_string(), path))
        } else {
            None
        }
    }

    pub fn from_read<T: Read>(&self, reader: T) -> Result<ID, io::Error> {
        let mut reader = BufReader::new(reader);
        create_dir_all(&self.root)?;
        let tempdir = TempDir::new_in(&self.root, "cas-scratch").unwrap();
        let tempfile = tempdir.path().join("cas");

        let mut digest = Sha256::new();
        debug!("writing CAS to {:?}", &tempfile);
        let mut f = BufWriter::new(File::create(&tempfile).unwrap());

        let mut buf = [0; 4096];
        loop {
            // loop duped from std::io::copy
            let len = match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(len) => len,
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };
            digest.input(&buf[..len]);
            f.write_all(&buf[..len])?;
        }

        let id = format!("{:x}", digest.result());
        let dest = self.root.join(&id);
        rename(&tempfile, &dest)?;

        Ok(ID(id, dest))
    }
}

pub struct ID(String, PathBuf);
impl ID {
    pub fn id(&self) -> &str {
        &self.0
    }
    pub fn as_path_buf(&self) -> PathBuf {
        self.1.clone()
    }
}
