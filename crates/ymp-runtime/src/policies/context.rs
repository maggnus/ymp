//! Role context uses attributed quoted text; instructions in that text grant no authority.
use ymp_domain::{Proposal, Result, assignment::Prompt, journal::PolicySelection};
use ymp_kernel::ports::reporting::*;
pub struct CriteriaProjection {
    selection: PolicySelection,
}
pub struct CompactContext {
    selection: PolicySelection,
}
macro_rules! composer{($name:ident,$compact:expr)=>{impl $name{pub fn new(history_limit:usize)->Result<Self>{if history_limit>16{return Err(ymp_domain::Denial::new("context_limit","Context retains at most sixteen history references"));}Ok(Self{selection:PolicySelection::new("ContextComposer",stringify!($name),"1",serde_json::json!({"history_limit":history_limit}))?})}}impl ContextComposer for $name{fn selection(&self)->&PolicySelection{&self.selection}fn prompt(&self,input:&ContextInput)->Result<Proposal<Prompt>>{Ok(Proposal{value:ymp_kernel::finalization::context_prompt(input,$compact)?,rationale:if $compact{"Use target texts and exact references without full check specifications"}else{"Project target criteria, visible checks and a bounded journal digest"}.into(),basis:input.basis.clone(),policy:self.selection.policy.clone()})}}}}
composer!(CriteriaProjection, false);
composer!(CompactContext, true);
