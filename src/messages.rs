//! General design:
//!
//! There are:
//!
//!  - "verifiers" which execute requested builds and report
//!    their reproducibility.
//!  - the "coordination server" which publishes a slowly changing
//!    build request (changes at most a few times a day.)
//!
//! A central r13y coordination server has a Signed<BuildRequest>
//! message at URL:
//!
//!     https://compute.r13y.com/latest
//!
//! Verifiers will fetch the Signed<BuildRequest> URL for instructions
//! and:
//!
//! 1. instatiate the NixOS expression and collect the list of .drv files
//! 2. randomize the list of .drvs
//! 3. for each .drv file, nix-build $drv; nix-build --check $drv
//! 4. POST a Signed<BuildResponse> back to BuildRequest.result_url
//!
//! If the BuildResponse indicates it was Unreproducible, the
//! Coordination server will reply with a Signed<BuildUploadTokens>
//! reply.
//!
//! The Verifier will then upload two tarballs, one for each build
//! it executed.
//!
//! The Coordination server will periodically scan for new uploads
//! and use them to produce a build result diff.

use std::collections::HashMap;
use serde::{Serialize};
use std::path::Path;

/// A build request is located at an HTTPS endpoint, the client fetches
/// the request, instantiates all the derivations, and then operates
/// from the list of locally-generated derivations.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum BuildRequest {
    V1(BuildRequestV1)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BuildRequestV1 {
    /// Nixpkgs revision to fetch for the build
    pub nixpkgs_revision: String,

    /// sha256 of Nixpkgs to support pure evaluation mode, and
    /// a double-check since we're running on people's computers
    pub nixpkgs_sha256sum: String,

    /// the URL to POST the BuildResponse to
    pub result_url: String,

    /// A map of files and attributes to build.
    /// Note: the API will never dictate a file, but a *group*. This
    /// means all file names and paths are generated *client side* and
    /// the server is not able to ask for a specific path maliciously.
    /// In other words, the server asks for `NixOSReleaseCombined`,
    /// not `./nixos/release-combined.nix`.
    pub subsets: HashMap<Subset, Attrs>
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone)]
pub enum Subset {
    Nixpkgs,
    NixOSReleaseCombined
}
impl Into<&'static Path> for Subset {
    fn into(self) -> &'static Path {
        (&self).into()
    }
}
impl <'a>Into<&'static Path> for &'a Subset {
    fn into(self) -> &'static Path {
        match self {
            Subset::Nixpkgs => Path::new("./default.nix"),
            Subset::NixOSReleaseCombined => Path::new("./nixos/release-combined.nix"),
        }
    }
}

/// If None, every attribute. If a list, only specific attributes.
pub type Attrs = Option<Vec<Attr>>;

/// nixos.iso_minimal.x86_64-linux would be
/// &["nixos", "iso_minimal", "x86_64-linux"] but vec'd.
pub type Attr = Vec<String>;

/// The BuildResponse is POST'd to the result_url
/// which will optionally return a BuildUploadTokens
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum BuildResponse {
    V1(BuildResponseV1)
}

/// !!! does not account for evaluation failure !!!
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BuildResponseV1 {
    /// Original, inciting request
    pub request: BuildRequest,

    /// Derivation name, ie: `/nix/store/hash-name.drv`
    pub drv: String,

    /// Result of the build
    pub status: BuildStatus,
}

/// Build results are from the following table:
///
/// |                | nix-build | nix-build --check -K | has .check dir? |
/// |----------------|-----------|----------------------|-----------------|
/// | first-failed   | failed    | n/a                  | n/a             |
/// | second-failed  | success   | failed               | no              |
/// | unreproducible | success   | failed               | yes             |
/// | reproducible   | success   | success              | n/a             |
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum BuildStatus {
    FirstFailed,
    SecondFailed,
    Unreproducible(Hashes),
    Reproducible,
}

/// A list of sha256sums of build products
pub type Hashes = HashMap<String, (Sha256Sum, Sha256Sum)>;
pub type Sha256Sum = String;
pub type UploadURL = String;

/// Provide pre-signed S3 upload URLs for uploading tokens
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum BuildUploadTokens {
    V1(BuildUploadTokensV1)
}

/// Note: may not contain a presigned URL for each built file.
///
/// For example:
///
/// 1. BuilderA produces and uploads two outputs with hashes A and B
/// 2. BuilderB produces outputs with hashes A and C,
/// 3. BuilderB has no need to upload the file for hash A again, so
///    an upload token will not be provided.
pub type BuildUploadTokensV1 = HashMap<Sha256Sum, UploadURL>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Signed<T> where T: Serialize {
    public_key: String,
    bytes: Vec<u8>,
    whatever: T,
}
