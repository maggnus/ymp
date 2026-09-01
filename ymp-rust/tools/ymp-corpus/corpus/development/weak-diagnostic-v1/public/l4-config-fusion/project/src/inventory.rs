use std::collections::BTreeMap;

pub fn normalize_inventory(lines: &[&str]) -> Result<BTreeMap<String, u64>, String> {
    let mut inventory = BTreeMap::new();
    for line in lines {
        let (service, revision) = line
            .split_once('=')
            .ok_or_else(|| "inventory line must contain =".to_owned())?;
        let service = service.trim();
        let revision = revision
            .trim()
            .parse::<u64>()
            .map_err(|_| "inventory revision must be an unsigned integer".to_owned())?;
        if service.is_empty() {
            return Err("inventory service must not be empty".to_owned());
        }
        inventory.insert(service.to_owned(), revision);
    }
    Ok(inventory)
}
