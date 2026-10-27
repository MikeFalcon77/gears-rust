use std::collections::BTreeMap;
#[derive(Debug)] pub struct Outbound { pub destination: String, pub headers: BTreeMap<String,String> }
/// The caller's debug label is public, unauthenticated correlation data; never used as identity.
pub fn public_request(destination: String, _label: String) -> Outbound { Outbound {destination,headers:BTreeMap::new()} }
/// destination comes from a user-supplied callback URL; service_secret authorizes internal administration.
pub fn callback(destination: String, _service_secret: String) -> Outbound { Outbound {destination,headers:BTreeMap::new()} }
