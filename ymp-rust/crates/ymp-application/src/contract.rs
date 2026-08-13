//! Turning a typed request into a contract a run can be started against.
//!
//! This is the single implementation behind every surface that starts a run: the terminal
//! interface calls it, and so must the equivalent command. It does two things and refuses
//! everything else.
//!
//! * [`prepare_contract`] validates a request, resolves what it names on this host, computes the
//!   oracle digest from the verifier program itself, and produces the exact bytes the contract
//!   will be stored as. It never invents the acceptance condition: a request without one is
//!   refused with the missing part named.
//! * [`Application::create_with_contract`] stores those bytes as an immutable object and starts
//!   the run, so the journal records the approved contract and the started run.

use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;
use ymp_domain::contract::{
    ContractDocument, ContractError, MissingPart, VerifierRecord, default_output_limit_bytes,
    default_wall_time_ms,
};
use ymp_domain::{Budget, Command, ContractBinding, digest_bytes};

use crate::{Application, ApplicationError, CommandOutcome};

/// The budget a run starts with when the request states none: one attempt to produce a candidate
/// and one verification query to judge it. Both dimensions are enforced by the domain.
pub const DEFAULT_RUN_BUDGET: Budget = Budget::new(1, 1);

/// How many hexadecimal characters of the request digest identify a drafted contract.
const DERIVED_IDENTIFIER_CHARS: usize = 12;

/// What the operator typed, plus what the kernel could not infer and therefore had to be given.
#[derive(Clone, Debug, Default)]
pub struct RunRequest {
    /// The work, in the operator's own words. It reaches the runtime unchanged.
    pub prompt: String,
    /// The directory the work is done in.
    pub source: PathBuf,
    /// What decides a candidate. `None` is a request that cannot start a run.
    pub acceptance: Option<AcceptanceCondition>,
    /// Paths excluded when the private workspace is captured, relative to the source.
    pub capture_exclusions: Vec<String>,
    /// The identifier of an existing package. A drafted request derives one from its own content.
    pub contract_id: Option<String>,
    /// The budget the run starts with, when the request states one.
    pub budget: Option<Budget>,
}

/// The acceptance condition as a request states it, before it is resolved on this host.
#[derive(Clone, Debug)]
pub struct AcceptanceCondition {
    pub program: PathBuf,
    pub arguments: Vec<String>,
    /// A deliberately wrong candidate the program must reject; a program that accepts it is not
    /// discriminating and cannot decide a run.
    pub negative_control: PathBuf,
    /// The digest a package declares for its oracle. It must identify the program named above;
    /// a drafted request declares none and the digest is read from the program itself.
    pub oracle_digest: Option<String>,
    pub wall_time_ms: u64,
    pub output_limit_bytes: usize,
}

impl AcceptanceCondition {
    /// A condition with the standard limits, which the operator may override.
    pub fn new(program: impl Into<PathBuf>, negative_control: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            arguments: Vec::new(),
            negative_control: negative_control.into(),
            oracle_digest: None,
            wall_time_ms: default_wall_time_ms(),
            output_limit_bytes: default_output_limit_bytes(),
        }
    }
}

/// A validated contract: the document, the exact bytes it is stored as, and the identifiers a
/// decision surface has to show before anything is spent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedContract {
    pub document: ContractDocument,
    pub contract_digest: String,
    pub budget: Budget,
    bytes: Vec<u8>,
}

impl PreparedContract {
    pub fn contract_id(&self) -> &str {
        &self.document.contract_id
    }

    pub fn oracle_digest(&self) -> &str {
        &self.document.verifier.oracle_digest
    }

    pub fn verifier(&self) -> &VerifierRecord {
        &self.document.verifier
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The identifier the run would carry. It is derived from the contract, so the operator can
    /// be shown it before the run exists and the same request never names two runs.
    pub fn run_id(&self) -> String {
        format!(
            "run-{}",
            &self.contract_digest[..DERIVED_IDENTIFIER_CHARS.min(self.contract_digest.len())]
        )
    }

    pub fn binding(&self) -> ContractBinding {
        ContractBinding {
            contract_id: self.document.contract_id.clone(),
            contract_digest: self.contract_digest.clone(),
            oracle_digest: self.document.verifier.oracle_digest.clone(),
        }
    }
}

#[derive(Debug, Error)]
pub enum ContractRequestError {
    #[error("no run started — the request states no {0}")]
    Missing(MissingPart),
    #[error("no run started — the source directory {path} could not be read: {reason}")]
    Source { path: PathBuf, reason: String },
    #[error("no run started — the source path is not a directory: {0}")]
    SourceNotADirectory(PathBuf),
    #[error("no run started — the verifier program {path} could not be read: {reason}")]
    VerifierProgram { path: PathBuf, reason: String },
    #[error("no run started — the verifier program is not an executable file: {0}")]
    VerifierNotExecutable(PathBuf),
    #[error("no run started — the negative control {path} could not be read: {reason}")]
    NegativeControl { path: PathBuf, reason: String },
    #[error("no run started — the negative control is not a directory: {0}")]
    NegativeControlNotADirectory(PathBuf),
    #[error(
        "no run started — the declared oracle digest {declared} does not identify the verifier \
         program {program}"
    )]
    OracleDigestMismatch { declared: String, program: PathBuf },
    #[error("no run started — {0}")]
    Contract(#[from] ContractError),
    #[error("no run started — the contract package {path} could not be read: {reason}")]
    Package { path: PathBuf, reason: String },
}

/// Validate a request and produce the contract it would be stored as.
///
/// Nothing is written and no budget is reserved here: the result is what a decision surface
/// shows before the operator authorizes the run.
pub fn prepare_contract(request: &RunRequest) -> Result<PreparedContract, ContractRequestError> {
    if request.prompt.trim().is_empty() {
        return Err(ContractRequestError::Missing(MissingPart::Prompt));
    }
    if request.source.as_os_str().is_empty() {
        return Err(ContractRequestError::Missing(MissingPart::Source));
    }
    let acceptance = request
        .acceptance
        .as_ref()
        .ok_or(ContractRequestError::Missing(
            MissingPart::AcceptanceCondition,
        ))?;
    if acceptance.program.as_os_str().is_empty() {
        return Err(ContractRequestError::Missing(MissingPart::VerifierProgram));
    }
    if acceptance.negative_control.as_os_str().is_empty() {
        return Err(ContractRequestError::Missing(MissingPart::NegativeControl));
    }

    let source = canonicalize(&request.source).map_err(|reason| ContractRequestError::Source {
        path: request.source.clone(),
        reason,
    })?;
    if !source.is_dir() {
        return Err(ContractRequestError::SourceNotADirectory(source));
    }

    let program = canonicalize(&acceptance.program).map_err(|reason| {
        ContractRequestError::VerifierProgram {
            path: acceptance.program.clone(),
            reason,
        }
    })?;
    let executable = program.is_file()
        && crate::answer::is_executable(&program).map_err(|error| {
            ContractRequestError::VerifierProgram {
                path: program.clone(),
                reason: error.to_string(),
            }
        })?;
    if !executable {
        return Err(ContractRequestError::VerifierNotExecutable(program));
    }
    // The oracle is identified by the exact program that will decide the run. A package that
    // declares a digest is held to it: a declaration that names something else would put a claim
    // in the stored contract that the program contradicts.
    let oracle_digest = digest_bytes(&fs::read(&program).map_err(|error| {
        ContractRequestError::VerifierProgram {
            path: program.clone(),
            reason: error.to_string(),
        }
    })?);
    if let Some(declared) = &acceptance.oracle_digest
        && declared != &oracle_digest
    {
        return Err(ContractRequestError::OracleDigestMismatch {
            declared: declared.clone(),
            program,
        });
    }

    let negative_control = canonicalize(&acceptance.negative_control).map_err(|reason| {
        ContractRequestError::NegativeControl {
            path: acceptance.negative_control.clone(),
            reason,
        }
    })?;
    if !negative_control.is_dir() {
        return Err(ContractRequestError::NegativeControlNotADirectory(
            negative_control,
        ));
    }

    let verifier = VerifierRecord {
        program,
        arguments: acceptance.arguments.clone(),
        negative_control,
        oracle_digest,
        wall_time_ms: acceptance.wall_time_ms,
        output_limit_bytes: acceptance.output_limit_bytes,
    };
    verifier.validate()?;

    let contract_id = match &request.contract_id {
        Some(identifier) => identifier.clone(),
        None => derive_contract_id(&request.prompt, &source, &verifier),
    };
    let document = ContractDocument {
        schema_version: ymp_domain::contract::CONTRACT_SCHEMA_VERSION,
        contract_id,
        source,
        prompt: request.prompt.clone(),
        capture_exclusions: request.capture_exclusions.clone(),
        verifier,
    };
    let bytes = document.canonical_bytes()?;
    Ok(PreparedContract {
        contract_digest: digest_bytes(&bytes),
        budget: request.budget.clone().unwrap_or(DEFAULT_RUN_BUDGET),
        document,
        bytes,
    })
}

/// Read a contract package from disk and validate it against this host.
///
/// A package that states no verifier, no negative control or no oracle digest is rejected here;
/// the same refusal a typed request receives when it states no acceptance condition. The stored
/// contract is the canonical document with its paths resolved, so its digest identifies what the
/// run is actually judged against rather than the file's formatting.
pub fn load_contract_package(
    path: impl AsRef<Path>,
) -> Result<PreparedContract, ContractRequestError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| ContractRequestError::Package {
        path: path.to_path_buf(),
        reason: error.to_string(),
    })?;
    let parsed = ContractDocument::parse(&bytes)?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let document = parsed.document;
    prepare_contract(&RunRequest {
        prompt: document.prompt,
        source: resolve(parent, document.source),
        acceptance: Some(AcceptanceCondition {
            program: resolve(parent, document.verifier.program),
            arguments: document.verifier.arguments,
            negative_control: resolve(parent, document.verifier.negative_control),
            oracle_digest: Some(document.verifier.oracle_digest),
            wall_time_ms: document.verifier.wall_time_ms,
            output_limit_bytes: document.verifier.output_limit_bytes,
        }),
        capture_exclusions: document.capture_exclusions,
        contract_id: Some(document.contract_id),
        budget: None,
    })
}

impl Application {
    /// Start a run against a prepared contract.
    ///
    /// The contract bytes become an immutable object, and the journal records the run start
    /// followed by the approval that binds the run to that exact contract and oracle. A store
    /// that already holds a run is not reused: `create` refuses it.
    pub fn create_with_contract(
        data_root: impl AsRef<Path>,
        contract: &PreparedContract,
    ) -> Result<(Self, CommandOutcome), ApplicationError> {
        let mut application = Self::create(data_root, contract.run_id(), contract.budget.clone())?;
        let stored = application.object_store.put(contract.bytes())?;
        if stored != contract.contract_digest {
            return Err(ApplicationError::ContractDigestMismatch);
        }
        let outcome = application.execute(
            format!("ymp.contract.approve.{}", contract.contract_id()),
            Command::ApproveContract {
                contract_id: contract.contract_id().to_owned(),
                contract_digest: contract.contract_digest.clone(),
                oracle_digest: contract.oracle_digest().to_owned(),
            },
        )?;
        Ok((application, outcome))
    }

    /// The contract this run is judged against, once one has been approved for it.
    pub fn contract(&self) -> Option<&ContractBinding> {
        self.state.contract.as_ref()
    }

    /// The stored bytes of the approved contract, read back from the immutable store.
    pub fn contract_bytes(&self) -> Result<Option<Vec<u8>>, ApplicationError> {
        match &self.state.contract {
            None => Ok(None),
            Some(binding) => Ok(Some(self.object_store.read(&binding.contract_digest)?)),
        }
    }
}

/// A contract drafted from a request is named by what it contains, so the same request always
/// produces the same identifier and the operator can be asked to type it before anything runs.
fn derive_contract_id(prompt: &str, source: &Path, verifier: &VerifierRecord) -> String {
    let material = format!(
        "{prompt}\u{0}{}\u{0}{}\u{0}{}\u{0}{}",
        source.display(),
        verifier.program.display(),
        verifier.negative_control.display(),
        verifier.oracle_digest
    );
    format!(
        "contract-{}",
        &digest_bytes(material.as_bytes())[..DERIVED_IDENTIFIER_CHARS]
    )
}

fn resolve(parent: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        parent.join(path)
    }
}

fn canonicalize(path: &Path) -> Result<PathBuf, String> {
    path.canonicalize().map_err(|error| error.to_string())
}
