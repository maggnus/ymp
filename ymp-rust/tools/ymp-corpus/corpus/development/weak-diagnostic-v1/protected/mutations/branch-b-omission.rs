use std::collections::BTreeMap;

pub fn normalize_routes(lines: &[&str]) -> Result<BTreeMap<String, String>, String> {
    let mut routes = BTreeMap::new();
    for line in lines {
        let (route, service) = line
            .split_once("->")
            .ok_or_else(|| "route line must contain ->".to_owned())?;
        let route = route.trim();
        let service = service.trim();
        if route.is_empty() || service.is_empty() {
            return Err("route and service must not be empty".to_owned());
        }
        routes.insert(route.to_owned(), service.to_owned());
    }
    Ok(routes)
}
