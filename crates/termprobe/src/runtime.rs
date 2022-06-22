use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, RwLock, Weak};

use sha2::{Digest, Sha256};

use crate::api::{
    LinkSelector, LocatorDirection, LocatorQuery, LocatorSelector, MatchOccurrence, MouseOptions,
    OpenOptions, OpenResult, Operation, OperationResult, RunOptions, StyleSelector, TextMatch,
    TextSelector, TermprobeError,
};
use crate::diagnostics::ExecutionContext;
use crate::engine::Engine;
use crate::logger::Logger;

const MAX_COMPLETED_RECORDINGS: usize = 1024;

#[derive(Clone)]
pub struct Session {
    name: Arc<str>,
    engine: Arc<Engine>,
    context: ExecutionContext,
}

#[derive(Debug, Clone, Copy)]
pub struct LocatorClickOptions {
    pub mouse: MouseOptions,
    pub clicks: u8,
    pub timeout_ms: Option<u64>,
}

impl Default for LocatorClickOptions {
    fn default() -> Self {
        Self {
            mouse: MouseOptions::default(),
            clicks: 1,
            timeout_ms: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LocatorExpectOptions {
    pub not: bool,
    pub timeout_ms: Option<u64>,
}

#[derive(Clone)]
enum LocatorTarget {
    Session(Session),
    Handle(SessionHandle),
}

impl LocatorTarget {
    fn same_owner(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Session(left), Self::Session(right)) => Arc::ptr_eq(&left.engine, &right.engine),
            (Self::Handle(left), Self::Handle(right)) => {
                Arc::ptr_eq(&left.registry.inner, &right.registry.inner) && left.name == right.name
            }
            _ => false,
        }
    }
    fn execute(
        &self,
        operation_name: &'static str,
        operation: Operation,
    ) -> Result<OperationResult, TermprobeError> {
        match self {
            Self::Session(session) => session.execute_named(operation_name, operation),
            Self::Handle(session) => session.execute_named(operation_name, operation),
        }
    }
}

/// A lazy query resolved against the current terminal grid before every read,
/// wait, or action.
#[derive(Clone)]
pub struct Locator {
    target: LocatorTarget,
    query: LocatorQuery,
}

/// Keep candidates containing `has` and containing no `has_not` matches.
#[derive(Clone, Default)]
pub struct LocatorFilterOptions {
    pub has: Option<Locator>,
    pub has_not: Option<Locator>,
}

impl Locator {
    fn new(target: LocatorTarget, query: LocatorQuery) -> Self {
        Self { target, query }
    }

    pub fn query(&self) -> &LocatorQuery {
        &self.query
    }

    fn require_same_owner(&self, other: &Self) -> Result<(), TermprobeError> {
        if self.target.same_owner(&other.target) {
            Ok(())
        } else {
            Err(TermprobeError::usage(
                "locator operands must belong to the same terminal owner",
            ))
        }
    }

    /// Intersect selected cells and form contiguous per-row runs.
    pub fn and(&self, other: &Self) -> Result<Self, TermprobeError> {
        self.require_same_owner(other)?;
        Ok(Self::new(
            self.target.clone(),
            self.query.clone().and(other.query.clone()),
        ))
    }

    /// Union selected cells and form contiguous per-row runs.
    pub fn or(&self, other: &Self) -> Result<Self, TermprobeError> {
        self.require_same_owner(other)?;
        Ok(Self::new(
            self.target.clone(),
            self.query.clone().or(other.query.clone()),
        ))
    }

    pub fn filter(&self, options: LocatorFilterOptions) -> Result<Self, TermprobeError> {
        if options.has.is_none() && options.has_not.is_none() {
            return Err(TermprobeError::usage("filter requires has or hasNot"));
        }
        for locator in options.has.iter().chain(options.has_not.iter()) {
            self.require_same_owner(locator)?;
        }
        Ok(Self::new(
            self.target.clone(),
            self.query.clone().filter(
                options.has.map(|locator| locator.query),
                options.has_not.map(|locator| locator.query),
            ),
        ))
    }

    /// Require every cell of each current match to have this link.
    pub fn get_by_link(&self, selector: impl Into<LinkSelector>) -> Self {
        self.get_by_link_relative(selector, LocatorDirection::Within)
    }

    pub fn get_by_link_relative(
        &self,
        selector: impl Into<LinkSelector>,
        direction: LocatorDirection,
    ) -> Self {
        Self::new(
            self.target.clone(),
            LocatorQuery {
                within: Some(Box::new(self.query.clone())),
                direction,
                ..LocatorQuery::link(selector)
            },
        )
    }

    pub fn get_by_text(&self, selector: impl Into<TextSelector>) -> Self {
        self.get_by_text_relative(selector, LocatorDirection::Within)
    }

    pub fn get_by_text_relative(
        &self,
        selector: impl Into<TextSelector>,
        direction: LocatorDirection,
    ) -> Self {
        Self {
            target: self.target.clone(),
            query: LocatorQuery {
                selector: LocatorSelector::Text(selector.into()),
                occurrence: MatchOccurrence::Any,
                within: Some(Box::new(self.query.clone())),
                direction,
                style: Default::default(),
            },
        }
    }

    pub fn get_by_style(&self, selector: impl Into<StyleSelector>) -> Self {
        self.get_by_style_relative(selector, LocatorDirection::Within)
    }

    pub fn get_by_style_relative(
        &self,
        selector: impl Into<StyleSelector>,
        direction: LocatorDirection,
    ) -> Self {
        Self {
            target: self.target.clone(),
            query: LocatorQuery {
                selector: LocatorSelector::Style(selector.into()),
                occurrence: MatchOccurrence::Any,
                within: Some(Box::new(self.query.clone())),
                direction,
                style: Default::default(),
            },
        }
    }

    pub fn any(&self) -> Self {
        self.with_occurrence(MatchOccurrence::Any)
    }

    pub fn unique(&self) -> Self {
        self.with_occurrence(MatchOccurrence::Unique)
    }

    pub fn first(&self) -> Self {
        self.with_occurrence(MatchOccurrence::First)
    }

    pub fn last(&self) -> Self {
        self.with_occurrence(MatchOccurrence::Last)
    }

    pub fn nth(&self, index: usize) -> Self {
        self.with_occurrence(MatchOccurrence::Nth(index))
    }

    fn with_occurrence(&self, occurrence: MatchOccurrence) -> Self {
        let mut locator = self.clone();
        locator.query.occurrence = occurrence;
        locator
    }

    pub fn all(&self) -> Result<Vec<Self>, TermprobeError> {
        let matches = self.locations()?;
        if self.query.occurrence == MatchOccurrence::Any {
            Ok((0..matches.len()).map(|index| self.nth(index)).collect())
        } else {
            Ok(matches.into_iter().map(|_| self.clone()).collect())
        }
    }

    pub fn count(&self) -> Result<usize, TermprobeError> {
        self.locations().map(|matches| matches.len())
    }

    pub fn locations(&self) -> Result<Vec<TextMatch>, TermprobeError> {
        match self.target.execute(
            "locator.find",
            Operation::FindLocator {
                query: self.query.clone(),
            },
        )? {
            OperationResult::Matches(matches) => Ok(matches),
            _ => Err(TermprobeError::internal(
                "locator locations returned an unexpected result type",
            )),
        }
    }

    pub fn location(&self) -> Result<TextMatch, TermprobeError> {
        match self.target.execute(
            "locator.location",
            Operation::ResolveLocator {
                query: self.query.clone(),
            },
        )? {
            OperationResult::Matches(mut matches) if matches.len() == 1 => Ok(matches.remove(0)),
            OperationResult::Matches(_) => Err(TermprobeError::internal(
                "locator location returned an invalid match count",
            )),
            _ => Err(TermprobeError::internal(
                "locator location returned an unexpected result type",
            )),
        }
    }

    pub fn wait(&self) -> Result<(), TermprobeError> {
        self.wait_with_timeout(None)
    }

    pub fn wait_with_timeout(&self, timeout_ms: Option<u64>) -> Result<(), TermprobeError> {
        self.wait_for(false, timeout_ms)
    }

    pub fn wait_hidden(&self, timeout_ms: Option<u64>) -> Result<(), TermprobeError> {
        self.wait_for(true, timeout_ms)
    }

