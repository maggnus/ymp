#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Event {
    pub id: String,
    pub predecessor: String,
    pub delta: u64,
}

pub fn decode(line: &str) -> Result<Event, String> {
    let mut fields = line.split('|');
    let id = fields.next().unwrap_or_default().trim();
    let predecessor = fields.next().unwrap_or_default().trim();
    let delta = fields
        .next()
        .ok_or_else(|| "event must contain three fields".to_owned())?
        .trim()
        .parse::<u64>()
        .map_err(|_| "delta must be an unsigned integer".to_owned())?;
    if fields.next().is_some() || id.is_empty() || predecessor.is_empty() || delta == 0 {
        return Err("event fields are invalid".to_owned());
    }
    Ok(Event {
        id: id.to_owned(),
        predecessor: predecessor.to_owned(),
        delta,
    })
}
