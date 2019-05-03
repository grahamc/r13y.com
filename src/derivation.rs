use serde_json;
use std;
use std::collections::HashMap;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Deserialize)]
pub struct Derivation {
    outputs: HashMap<String, HashMap<String, PathBuf>>,
    // inputSrcs,
    // inputDrvs,
    // platform,
    // builder,
    // args,
    // env: HashMap<String, String>,
}

impl Derivation {
    pub fn parse(drv: &Path) -> Result<Derivation, DerivationParseError> {
        let mut drvs = Derivation::parse_many(&[drv])?;
        let key = drv.to_str().unwrap();

        match drvs.remove(key) {
            Some(parsed) => Ok(parsed),
            None => Err(DerivationParseError::NotInResult),
        }
    }

    pub fn parse_many(drvs: &[&Path]) -> Result<HashMap<String, Derivation>, DerivationParseError> {
        debug!("Parsing derivations: {:#?}", &drvs);
        let show = Command::new("nix")
            .arg("show-derivation")
            .args(drvs.iter())
            .output()?;
        for line in show.stderr.lines() {
            debug!("parse derivation stderr: {:?}", line);
        }
        Ok(serde_json::from_slice(&show.stdout)?)
    }

    pub fn outputs(&self) -> HashMap<&String, &PathBuf> {
        self.outputs
            .iter()
            .map(|(name, submap)| (name, submap.get("path")))
            .filter(|(_, path)| path.is_some())
            .map(|(name, path)| (name, path.unwrap()))
            .collect()
    }
}

#[derive(Debug)]
pub enum DerivationParseError {
    Io(std::io::Error),
    JsonDecode(serde_json::Error),
    NotInResult,
}

impl From<serde_json::Error> for DerivationParseError {
    fn from(e: serde_json::Error) -> Self {
        DerivationParseError::JsonDecode(e)
    }
}

impl From<std::io::Error> for DerivationParseError {
    fn from(e: std::io::Error) -> Self {
        DerivationParseError::Io(e)
    }
}
