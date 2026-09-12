//! Replaceable selection heuristics. All returned proposals pass runtime checks.
use anyhow::Result;
use ymp_core::*;

pub trait AllocationPolicy: Send + Sync {
    fn identity(&self) -> ExecutionBackendIdentity;
    fn propose(&self, input: &AllocationInput) -> Result<AllocationProposal>;
}

pub trait ResourceAllocationPolicy: Send + Sync {
    fn identity(&self) -> ExecutionBackendIdentity;
    fn propose(&self, input: &ResourceAllocationInput) -> Result<InvocationAllowance>;
}

pub struct BoundedAllocationPolicy;
impl AllocationPolicy for BoundedAllocationPolicy {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "ymp.bounded-allocation".into(),
            version: "2".into(),
        }
    }
    fn propose(&self, input: &AllocationInput) -> Result<AllocationProposal> {
        let reserved = input
            .current
            .as_ref()
            .and_then(|s| s.reserved_final_reviewer.as_ref())
            .filter(|id| {
                input.eligible.iter().any(|a| &a.id == *id) && !input.producer_ids.contains(id)
            })
            .cloned()
            .or_else(|| {
                input
                    .eligible
                    .iter()
                    .rev()
                    .find(|a| !input.producer_ids.contains(&a.id))
                    .map(|a| a.id.clone())
            });
        let mut options = input
            .candidates
            .iter()
            .filter(|c| input.demand.purpose != "execute" || Some(&c.agent_id) != reserved.as_ref())
            .collect::<Vec<_>>();
        // Only qualified, configuration-specific evidence changes preference.
        // Native effort names have no ordering, and failed work adds no effort increment.
        options.sort_by(|a, b| {
            b.experience
                .mean()
                .total_cmp(&a.experience.mean())
                .then_with(|| {
                    let current = |id: &str| {
                        input
                            .current
                            .as_ref()
                            .is_some_and(|s| s.current_members.iter().any(|member| member == id))
                    };
                    current(&b.agent_id).cmp(&current(&a.agent_id))
                })
        });
        let executor = if input.demand.ready_work == 0 {
            None
        } else {
            options.first().cloned().cloned()
        };
        if input.demand.ready_work > 0 && executor.is_none() {
            anyhow::bail!("no_independent_eligible_reviewer: no executor remains while preserving independent final review");
        }
        let minimum =
            if input.demand.difficulty == "complex" || input.demand.risk == TaskRisk::Elevated {
                (input
                    .demand
                    .ready_work
                    .min(input.budget.as_ref().map_or(1, |b| b.limits.parallel))
                    + 1)
                .clamp(2, 3)
            } else {
                2
            };
        // Ordinary independent work can justify concurrent producers too. This
        // bounded width is a heuristic; it does not claim optimal utilization.
        let useful_width = if input.demand.purpose == "execute" {
            (input.demand.ready_work + input.occupied_agent_ids.len())
                .min(input.budget.as_ref().map_or(1, |b| b.limits.parallel))
        } else {
            0
        };
        let minimum = minimum
            .max(useful_width + 1)
            .max(input.occupied_agent_ids.len());
        let target = input
            .constraints
            .fixed_roster
            .as_ref()
            .map(Vec::len)
            .or(input.constraints.fixed_size)
            .unwrap_or(
                minimum
                    .min(input.constraints.max_members)
                    .min(input.eligible.len()),
            );
        let mut members = input.constraints.fixed_roster.clone().unwrap_or_default();
        if input.constraints.fixed_roster.is_none() {
            members.extend(input.occupied_agent_ids.iter().cloned());
            if let Some(choice) = &executor {
                if !members.contains(&choice.agent_id) {
                    members.push(choice.agent_id.clone());
                }
            }
            anyhow::ensure!(members.len() <= target, "active_responsibility: selected work cannot displace occupied members within the captured ceiling");
            if let Some(id) = &reserved {
                if members.len() < target && !members.contains(id) {
                    members.push(id.clone());
                }
            }
            for agent in &input.eligible {
                if members.len() >= target {
                    break;
                }
                if !members.contains(&agent.id) {
                    members.push(agent.id.clone());
                }
            }
        }
        Ok(AllocationProposal {
            members, executor, reserved_final_reviewer: reserved,
            method: if input.demand.difficulty == "complex" || input.demand.risk == TaskRisk::Elevated { "decompose_and_check" } else { "direct_and_check" }.into(),
            rationale: format!("Bounded {} work at {:?}; complexity={}, risk={:?}, ready={}; preserve an independent final reviewer; rank exact native configurations by qualified experience. Failure requires reconsidering evidence, not automatic escalation.", input.demand.purpose, input.boundary, input.demand.difficulty, input.demand.risk, input.demand.ready_work),
        })
    }
}

pub struct BoundedResourcePolicy;
impl ResourceAllocationPolicy for BoundedResourcePolicy {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "ymp.bounded-resources".into(),
            version: "1".into(),
        }
    }
    fn propose(&self, input: &ResourceAllocationInput) -> Result<InvocationAllowance> {
        let defaults = ResourceLimits::default();
        let resources = input.budget.limits.resources.as_ref().unwrap_or(&defaults);
        let native_turns = if startup_purpose(&input.demand.purpose) {
            4
        } else if input.demand.difficulty == "complex" || input.demand.risk == TaskRisk::Elevated {
            16
        } else {
            8
        };
        Ok(InvocationAllowance {
            timeout_secs: input.budget.limits.turn_timeout_secs,
            native_max_turns: resources.native_max_turns.min(native_turns),
            max_output_chars: resources.max_output_chars,
            rationale: "Bound native work by purpose, complexity and risk within captured ceilings; session admission retains the verification reserve and historical spend.".into(),
        })
    }
}
