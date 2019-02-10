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


/// A build request is located at an HTTPS endpoint, the client fetches
/// the request, instantiates all the derivations, and then operates
/// from the list of locally-generated derivations.
enum BuildRequest {
    V1(BuildRequestV1)
}


struct BuildRequestV1 {
    /// Nixpkgs revision to fetch for the build
    nixpkgs_revision: str,

    /// sha256 of Nixpkgs to support pure evaluation mode, and
    /// a double-check since we're running on people's computers
    nixpkgs_sha256sum: str,

    /// the URL to POST the BuildResponse to
    result_url: str,

    /// A list of files and attributes to build.
    /// Note: the API will never dictate a file, but a *group*. This
    /// means all file names and paths are generated *client side* and
    /// the server is not able to ask for a specific path maliciously.
    /// In other words, the server asks for `NixOSReleaseCombined`,
    /// not `./nixos/release-combined.nix`.
    subsets: Vec<Subset>
}

enum Subset {
    NixOSReleaseCombined(Attrs)
}

/// If None, every attribute. If a list, only specific attributes.
type Attrs = Option<Vec<str>>;




/// The BuildResponse is POST'd to the result_url
/// which will optionally return a BuildUploadTokens
enum BuildResponse {
    V1(BuildResponseV1)
}

/// !!! does not account for evaluation failure !!!
struct BuildResponseV1 {
    /// Original, inciting request
    request: BuildRequest,

    /// Derivation name, ie: `/nix/store/hash-name.drv`
    drv: str,

    /// Result of the build
    status: BuildStatus,
}

/// Build results are from the following table:
///
/// |                | nix-build | nix-build --check -K | has .check dir? |
/// |----------------|-----------|----------------------|-----------------|
/// | first-failed   | failed    | n/a                  | n/a             |
/// | second-failed  | success   | failed               | no              |
/// | unreproducible | success   | failed               | yes             |
/// | reproducible   | success   | success              | n/a             |
enum BuildStatus {
    FirstFailed,
    Secondfailed,
    Unreproducible(Hashes),
    Reproducible,
}

/// A list of sha256sums of build products
type Hashes = Vec<Sha256Sum>;
type Sha256Sum = str;
type UploadURL = str;

/// Provide pre-signed S3 upload URLs for uploading tokens
enum BuildUploadTokens {
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
type BuildUploadTokensV1 = HashMap<(Sha256Sum, UploadURL)>;


struct Signed<T> where T: Serializable {
    public_key: str,
    bytes: Vec<u8>
}
