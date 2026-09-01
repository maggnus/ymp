mod chain;
mod parse;

use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Receipt {
    pub total: u64,
    pub head: String,
}

#[derive(Default)]
pub struct Ledger {
    total: u64,
    head: Option<String>,
    accepted: BTreeMap<String, Receipt>,
}

impl Ledger {
    pub fn apply_line(&mut self, line: &str) -> Result<Receipt, String> {
        let event = parse::decode(line)?;
        if let Some(receipt) = self.accepted.get(&event.id) {
            return Ok(receipt.clone());
        }
        let predecessor = self.head.as_deref().unwrap_or("GENESIS");
        if event.predecessor != predecessor {
            return Err("predecessor does not match committed head".to_owned());
        }
        let total = self
            .total
            .checked_add(event.delta)
            .ok_or_else(|| "total overflow".to_owned())?;
        let head = chain::next_head(predecessor, &event.id, total);
        let receipt = Receipt { total, head };
        self.total = total;
        self.head = Some(receipt.head.clone());
        self.accepted.insert(event.id, receipt.clone());
        Ok(receipt)
    }

    pub fn state(&self) -> (u64, Option<&str>) {
        (self.total, self.head.as_deref())
    }
}
