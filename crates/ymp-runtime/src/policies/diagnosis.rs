//! Exact A9 first-match diagnosis and a deterministic conservative control.
use ymp_domain::{Proposal, Result, journal::PolicySelection, verification::Diagnosis};
use ymp_kernel::ports::progress::*;
pub struct RuleBasedDiagnoser {
    selection: PolicySelection,
    parameters: DiagnosisParameters,
}
pub struct DirectFailuresOnly {
    selection: PolicySelection,
    parameters: DiagnosisParameters,
}
macro_rules! diagnoser{($name:ident,$direct:expr)=>{impl $name{pub fn new(parameters:DiagnosisParameters)->Result<Self>{Ok(Self{selection:PolicySelection::new("FailureDiagnoser",stringify!($name),"1",serde_json::json!(parameters))?,parameters})}}impl FailureDiagnoser for $name{fn selection(&self)->&PolicySelection{&self.selection}fn diagnose(&self,input:&DiagnosisInput)->Result<Proposal<Diagnosis>>{Ok(Proposal{value:diagnose(input,&self.parameters,$direct),rationale:if $direct{"Experimental control recognizes direct execution/check defects and requests diagnosis for inference"}else{"Apply the approved A9 first-match order to applicable recorded facts"}.into(),basis:input.basis.clone(),policy:self.selection.policy.clone()})}}}}
diagnoser!(RuleBasedDiagnoser, false);
diagnoser!(DirectFailuresOnly, true);
