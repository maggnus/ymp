pub mod inventory;
pub mod routing;

pub fn release_plan(inventory_lines: &[&str], route_lines: &[&str]) -> Result<Vec<String>, String> {
    let inventory = inventory::normalize_inventory(inventory_lines)?;
    let routes = routing::normalize_routes(route_lines)?;
    Ok(routes
        .into_iter()
        .filter_map(|(route, service)| {
            inventory
                .get(&service)
                .map(|revision| format!("{route}@{service}#{revision}"))
        })
        .collect())
}
