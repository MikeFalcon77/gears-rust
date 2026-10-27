use crate::ledger::Ledger;
/// Billing provider retries webhook events after network timeouts. The event ID must reach ledger deduplication.
pub fn charge_webhook(event: &str, amount: i64, ledger: &mut Ledger) { ledger.charge(Some(event), amount); }
use std::collections::BTreeMap;
pub fn attach_tag(headers: &mut BTreeMap<String,String>, tag: Option<&str>) {
    if let Some(tag)=tag { headers.entry("x-debug-tag".into()).or_insert(tag.into()); }
}
pub fn outbound(tag: Option<&str>) -> (u16,BTreeMap<String,String>) {
    let mut headers=BTreeMap::new(); attach_tag(&mut headers,tag); (200,headers)
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn charge_once() { let mut ledger=Ledger::default(); charge_webhook("evt",5,&mut ledger); assert_eq!(ledger.balance,-5); }
    #[test] fn tag_none() { let mut headers=BTreeMap::new(); attach_tag(&mut headers,None); assert!(headers.is_empty()); }
    #[test] fn tag_present() { let mut headers=BTreeMap::new(); attach_tag(&mut headers,Some("x")); assert_eq!(headers.get("x-debug-tag").unwrap(),"x"); }
    #[test] fn outbound_present() { assert_eq!(outbound(Some("x")).1.get("x-debug-tag").unwrap(),"x"); }
    #[test] fn outbound_absent() { assert_eq!(outbound(None).0,200); }
}
