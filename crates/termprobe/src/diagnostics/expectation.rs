use serde::{Deserialize, Serialize};

use crate::api::{ClipboardPattern, LocatorQuery, Operation};

const MAX_EXPECTATION_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocatorExpectation {
    Matches,
    Visible,
    Hidden,
    Unique,
    Actionable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OperationExpectation {
    Locator {
        query: Box<LocatorQuery>,
        outcome: LocatorExpectation,
    },
    Value {
        subject: String,
        expected: String,
    },
    Unavailable {
        reason: String,
    },
}

impl OperationExpectation {
    pub(crate) fn capture(operation: &Operation) -> Option<Self> {
        let value = |subject: &str, expected: String| Self::Value {
            subject: subject.into(),
            expected,
        };
        let pattern = |text: &str, regex: bool| {
            format!(
                "{} {text:?}",
                if regex { "matches regex" } else { "contains" }
            )
        };
        let expectation = match operation {
            Operation::FindLocator { query } => Self::Locator {
                query: Box::new(query.clone()),
                outcome: LocatorExpectation::Matches,
            },
            Operation::WaitLocator { query, not, .. } => Self::Locator {
                query: Box::new(query.clone()),
                outcome: if *not {
                    LocatorExpectation::Hidden
                } else {
                    LocatorExpectation::Visible
                },
            },
            Operation::ResolveLocator { query } => Self::Locator {
                query: Box::new(query.clone()),
                outcome: LocatorExpectation::Unique,
            },
            Operation::ClickLocator { query, .. } => Self::Locator {
                query: Box::new(query.clone()),
