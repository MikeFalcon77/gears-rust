use std::collections::HashSet;
#[derive(Default)] pub struct Ledger { pub balance: i64, processed: HashSet<String> }
impl Ledger {
    pub fn charge(&mut self, event: Option<&str>, amount: i64) {
        if let Some(event) = event { if !self.processed.insert(event.into()) { return; } }
        self.balance -= amount;
    }
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn store_replays_are_idempotent() {
        let mut s=Ledger::default(); s.charge(Some("evt"),5); s.charge(Some("evt"),5);
        assert_eq!(s.balance,-5);
    }
}
