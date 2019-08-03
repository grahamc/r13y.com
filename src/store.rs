use std::{
    io::{self, BufRead, Read},
    path::{Path, PathBuf},
    process::{Child, ChildStderr, ChildStdout, Command, Output, Stdio},
};

pub struct Store {}

impl Store {
    pub fn new() -> Store {
        Store {}
    }

    pub fn create_gc_root(&self, store_path: &Path, gc_root: &Path) -> Result<(), RealiseError> {
        let realise = Command::new("nix-store")
            .arg("--add-root")
            .arg(&gc_root)
            .arg("--indirect")
            .arg("--realise")
            .arg(&store_path)
            .stdin(Stdio::null())
            .output()?;
        if realise.status.success() {
            Ok(())
        } else {
            Err(RealiseError::Failed(realise))
        }
    }

    pub fn add_path(&self, path: &Path, gc_root: &Path) -> Result<PathBuf, AddToStoreError> {
        let add_cmd = Command::new("nix").arg("add-to-store").arg(path).output()?;

        self.debug_stderr(add_cmd.stderr);
        if !add_cmd.status.success() {
            panic!("there");
        }

        let mut lines: Vec<Result<String, _>> = add_cmd.stdout.lines().collect();
        if lines.len() != 1 {
            return Err(AddToStoreError::TooManyLines(lines));
        }

        let line = lines.pop().expect("Just verified one line above")?;

        let path = PathBuf::from(line);
        self.create_gc_root(&path, &gc_root)?;
        Ok(path)
    }

    ///
    pub fn export_nar(
        &self,
        path: &Path,
    ) -> Result<(ChildStdout, ExportNarWait), ExportNarStartError> {
        let mut add_cmd = Command::new("nix")
            .arg("dump-path")
            .arg(path)
            .stdin(Stdio::null())
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        Ok((
            add_cmd.stdout.take().unwrap(),
            ExportNarWait {
                stderr: add_cmd.stderr.take().unwrap(),
                child: add_cmd,
            },
        ))
    }

    fn debug_stderr(&self, stderr: Vec<u8>) {
        for line in stderr.lines() {
            debug!("stderr: {:?}", line)
        }
    }
}

#[derive(Debug)]
pub enum RealiseError {
    Io(io::Error),
    Failed(Output),
}
impl From<io::Error> for RealiseError {
    fn from(e: io::Error) -> RealiseError {
        RealiseError::Io(e)
    }
}

#[derive(Debug)]
pub enum AddToStoreError {
    Io(io::Error),
    TooManyLines(Vec<Result<String, io::Error>>),
    Realise(RealiseError),
}
impl From<io::Error> for AddToStoreError {
    fn from(e: io::Error) -> AddToStoreError {
        AddToStoreError::Io(e)
    }
}
impl From<RealiseError> for AddToStoreError {
    fn from(e: RealiseError) -> AddToStoreError {
        AddToStoreError::Realise(e)
    }
}

#[derive(Debug)]
pub enum ExportNarStartError {
    Io(io::Error),
}
impl From<io::Error> for ExportNarStartError {
    fn from(e: io::Error) -> ExportNarStartError {
        ExportNarStartError::Io(e)
    }
}

pub struct ExportNarWait {
    child: Child,
    stderr: ChildStderr,
}

impl ExportNarWait {
    pub fn wait(&mut self) -> Result<(), ExportNarFinishError> {
        let result = self.child.wait()?;

        if result.success() {
            Ok(())
        } else {
            let mut stderr = String::new();
            self.stderr.read_to_string(&mut stderr)?;
            Err(ExportNarFinishError::Failed(result.code(), stderr))
        }
    }
}

#[derive(Debug)]
pub enum ExportNarFinishError {
    Io(io::Error),
    Failed(Option<i32>, String),
}
impl From<io::Error> for ExportNarFinishError {
    fn from(e: io::Error) -> ExportNarFinishError {
        ExportNarFinishError::Io(e)
    }
}
