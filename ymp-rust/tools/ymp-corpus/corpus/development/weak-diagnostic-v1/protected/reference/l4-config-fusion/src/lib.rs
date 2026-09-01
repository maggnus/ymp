pub mod inventory;
pub mod routing;

pub fn release_plan(inventory_lines: &[&str], route_lines: &[&str]) -> Result<Vec<String>, String> {
    let inventory = inventory::normalize_inventory(inventory_lines)?;
    let routes = routing::normalize_routes(route_lines)?;
    for service in routes.values() {
        if !inventory.contains_key(service) {
            return Err(format!("route targets absent service {service}"));
        }
    }
    for service in inventory.keys() {
        if !routes.values().any(|target| target == service) {
            return Err(format!("inventory service {service} has no route"));
        }
    }
    Ok(routes
        .into_iter()
        .map(|(route, service)| {
            let revision = inventory[&service];
            format!("{route}@{service}#{revision}")
        })
        .collect())
}
