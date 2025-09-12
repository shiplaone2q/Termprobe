//! Typed, non-recursive expression transport shared by the native bindings.

use serde::{Deserialize, Serialize};

use crate::api::{
    LinkSelector, LocatorDirection, LocatorQuery, LocatorSelector, MatchOccurrence, StyleSelector,
    TextSelector, TextStyle, TermprobeError, WhitespaceMode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocatorNodeKind {
    Text,
    Style,
    Link,
    And,
    Or,
    Filter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocatorNode {
    pub kind: LocatorNodeKind,
    pub text: Option<String>,
    pub regex: Option<bool>,
    pub whitespace: Option<WhitespaceMode>,
    pub style: Option<TextStyle>,
    pub link: Option<String>,
    pub full: Option<bool>,
    pub direction: Option<LocatorDirection>,
    pub within: Option<usize>,
    pub left: Option<usize>,
    pub right: Option<usize>,
    pub input: Option<usize>,
    pub has: Option<usize>,
    pub has_not: Option<usize>,
    pub occurrence: MatchOccurrence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocatorExpression {
    pub nodes: Vec<LocatorNode>,
    pub root: usize,
}

impl LocatorExpression {
    pub fn into_query(self) -> Result<LocatorQuery, TermprobeError> {
        if self.nodes.is_empty() || self.nodes.len() > 256 || self.root >= self.nodes.len() {
            return Err(TermprobeError::usage(
                "locator expression requires 1..256 nodes and a valid root",
            ));
        }

        let mut depths: Vec<usize> = Vec::new();
        let mut sizes: Vec<usize> = Vec::new();
        for (index, node) in self.nodes.iter().enumerate() {
            let error = |message: &str| TermprobeError::usage(format!("nodes[{index}]: {message}"));
            let references = [
                node.within,
                node.left,
                node.right,
                node.input,
                node.has,
                node.has_not,
            ];
            let mut depth = 1;
            let mut size = 1;
            for reference in references.into_iter().flatten() {
                if reference >= index {
                    return Err(error("operand references must point to earlier nodes"));
                }
                depth = depth.max(depths[reference] + 1);
                size += sizes[reference];
            }
            if depth > 64 || size > 4096 {
                return Err(error("locator expression exceeds the size or depth limit"));
            }
            let binary = matches!(node.kind, LocatorNodeKind::And | LocatorNodeKind::Or);
            let filter = node.kind == LocatorNodeKind::Filter;
            let leaf = !binary && !filter;
            if (node.kind != LocatorNodeKind::Text
                && (node.text.is_some() || node.regex.is_some() || node.whitespace.is_some()))
                || (node.kind != LocatorNodeKind::Style && node.style.is_some())
                || (node.kind != LocatorNodeKind::Link && node.link.is_some())
                || (!leaf
                    && (node.full.is_some() || node.within.is_some() || node.direction.is_some()))
                || (!binary && (node.left.is_some() || node.right.is_some()))
                || (!filter
                    && (node.input.is_some() || node.has.is_some() || node.has_not.is_some()))
            {
                return Err(error("fields do not match locator node kind"));
