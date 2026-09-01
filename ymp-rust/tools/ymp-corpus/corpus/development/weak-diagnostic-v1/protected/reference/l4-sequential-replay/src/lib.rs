mod chain;
mod parse;

use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Receipt {
    pub total: u64,
    pub head: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum StoredResult {
    Accepted(Receipt),
    Rejected(String),
}

#[derive(Default)]
pub struct Ledger {
    total: u64,
    head: Option<String>,
    outcomes: BTreeMap<String, StoredResult>,
}

impl Ledger {
    pub fn apply_line(&mut self, line: &str) -> Result<Receipt, String> {
        let event = parse::decode(line)?;
        if let Some(outcome) = self.outcomes.get(&event.id) {
            return match outcome {
                StoredResult::Accepted(receipt) => Ok(receipt.clone()),
                StoredResult::Rejected(reason) => Err(reason.clone()),
            };
        }
        let result = self.apply_new(&event);
        let stored = match &result {
            Ok(receipt) => StoredResult::Accepted(receipt.clone()),
            Err(reason) => StoredResult::Rejected(reason.clone()),
        };
        self.outcomes.insert(event.id, stored);
        result
    }

    fn apply_new(&mut self, event: &parse::Event) -> Result<Receipt, String> {
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
        Ok(receipt)
    }

    pub fn state(&self) -> (u64, Option<&str>) {
        (self.total, self.head.as_deref())
    }
}
