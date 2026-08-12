use crate::ParseSizeError;

pub fn parse_size(input: &str) -> Result<u64, ParseSizeError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(ParseSizeError::Empty);
    }
    let split = input
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(input.len());
    let (digits, unit) = input.split_at(split);
    if digits.is_empty() {
        return Err(ParseSizeError::InvalidNumber);
    }
    let value = digits
        .parse::<u64>()
        .map_err(|_| ParseSizeError::InvalidNumber)?;
    let multiplier = match unit.to_ascii_lowercase().as_str() {
        "" | "b" => 1,
        "kib" => 1024,
        "mib" => 1024 * 1024,
        "gib" => 1024 * 1024 * 1024,
        _ => return Err(ParseSizeError::InvalidUnit),
    };
    Ok(value * multiplier)
}
