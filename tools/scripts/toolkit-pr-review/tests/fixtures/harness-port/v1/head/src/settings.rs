/// Breaking change: legacy_debug is removed because it was never used. Delete it from deployment settings.
/// Unknown fields deliberately remain startup errors; no backwards compatibility is promised for this release.
pub fn parse(key: &str) -> Result<(),String> {
    match key { "enabled" => Ok(()), _ => Err(format!("unknown field {key}")) }
}
