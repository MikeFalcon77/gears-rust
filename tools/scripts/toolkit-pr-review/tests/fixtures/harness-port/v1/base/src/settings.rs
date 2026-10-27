/// legacy_debug is accepted but has never been consulted by runtime code.
pub fn parse(key: &str) -> Result<(),String> {
    match key { "enabled" | "legacy_debug" => Ok(()), _ => Err(format!("unknown field {key}")) }
}
