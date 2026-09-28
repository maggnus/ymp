//! Paid narration and the zero-call deterministic facts fallback share one consumer.
use ymp_domain::{Proposal, Result, journal::PolicySelection};
use ymp_kernel::{finalization::NarrativeParameters, ports::reporting::*};
pub struct Narrator {
    selection: PolicySelection,
}
pub struct DeterministicReport {
    selection: PolicySelection,
}
macro_rules! composer{($name:ident,$paid:expr)=>{impl $name{pub fn new()->Result<Self>{Ok(Self{selection:PolicySelection::new("NarrativeComposer",stringify!($name),"1",serde_json::json!(NarrativeParameters{max_claims:128,max_text:4096}))?})}}impl NarrativeComposer for $name{fn selection(&self)->&PolicySelection{&self.selection}fn compose(&self,input:&NarrativeInput)->Result<Proposal<ReportDraft>>{let value=if $paid{ymp_domain::journal::decode(input.source.as_ref().ok_or_else(||ymp_domain::Denial::new("narrator_unavailable","No paid completed narrator output"))?.as_bytes())?}else{ymp_kernel::finalization::deterministic(input)?};Ok(Proposal{value,rationale:if $paid{"Use the exact paid Narrator output; claims remain subject to independent kernel audit"}else{"Explain recorded facts and uncertainty without another model call"}.into(),basis:input.basis.clone(),policy:self.selection.policy.clone()})}}}}
composer!(Narrator, true);
composer!(DeterministicReport, false);
