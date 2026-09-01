use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuotaPatch {
    pub service: String,
    pub limit: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutePatch {
    pub path: String,
    pub service: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Plan {
    pub quotas: BTreeMap<String, u32>,
    pub routes: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PolicyError {
    QuotaConflict(String),
    RouteConflict(String),
    MissingQuota(String),
}

pub fn compile_plan(
    quotas: impl IntoIterator<Item = QuotaPatch>,
    routes: impl IntoIterator<Item = RoutePatch>,
) -> Result<Plan, PolicyError> {
    let mut normalized_quotas = BTreeMap::new();
    for patch in quotas {
        if let Some(previous) = normalized_quotas.insert(patch.service.clone(), patch.limit)
            && previous != patch.limit
        {
            return Err(PolicyError::QuotaConflict(patch.service));
        }
    }
    let mut normalized_routes = BTreeMap::new();
    for patch in routes {
        if let Some(previous) = normalized_routes.insert(patch.path.clone(), patch.service.clone())
            && previous != patch.service
        {
            return Err(PolicyError::RouteConflict(patch.path));
        }
    }
    for service in normalized_routes.values() {
        if !normalized_quotas.contains_key(service) {
            return Err(PolicyError::MissingQuota(service.clone()));
        }
    }
    Ok(Plan {
        quotas: normalized_quotas,
        routes: normalized_routes,
    })
}
