pub mod inventory;
pub mod routing;

pub fn release_plan(inventory_lines: &[&str], route_lines: &[&str]) -> Result<Vec<String>, String> {
    let inventory = inventory::normalize_inventory(inventory_lines)?;
    let routes = routing::normalize_routes(route_lines)?;
    let mut plan = Vec::new();
    for (route, service) in routes {
        if let Some(revision) = inventory.get(&service) {
            plan.push(format!("{route}@{service}#{revision}"));
        }
    }
    Ok(plan)
}
