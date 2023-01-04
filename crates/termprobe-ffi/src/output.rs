use crate::types::*;
use termprobe::api::*;

include!("output_generated.rs");

#[derive(Default)]
struct Storage {
    bytes: Vec<Box<[u8]>>,
    cells: Vec<TuiCell>,
    matches: Vec<TuiMatch>,
    spans: Vec<Box<[TuiSpan]>>,
    bells: Vec<TuiBellEvent>,
    strings: Vec<TuiString>,
    modes: Vec<TuiMode>,
    pairs: Vec<TuiPair>,
}
impl Storage {
    fn bytes(&mut self, value: Vec<u8>) -> TuiString {
        let value = value.into_boxed_slice();
        let output = TuiString {
            data: value.as_ptr(),
            len: value.len(),
        };
        self.bytes.push(value);
        output
    }
    fn string(&mut self, value: String) -> TuiString {
        self.bytes(value.into_bytes())
    }
    fn optional(&mut self, value: Option<String>) -> TuiString {
        value.map(|s| self.string(s)).unwrap_or_default()
    }
    fn modes(
        &mut self,
        values: std::collections::BTreeMap<String, bool>,
    ) -> (*const TuiMode, usize) {
        for (name, enabled) in values {
            let name = self.string(name);
            self.modes.push(TuiMode { name, enabled });
        }
        (self.modes.as_ptr(), self.modes.len())
    }
    fn palette(
        &mut self,
        values: std::collections::BTreeMap<u8, String>,
    ) -> (*const TuiPair, usize) {
        for (index, color) in values {
            let key = self.string(index.to_string());
            let value = self.string(color);
            self.pairs.push(TuiPair { key, value });
        }
        (self.pairs.as_ptr(), self.pairs.len())
    }
    fn spans(&mut self, values: Vec<TextSpan>) -> (*const TuiSpan, usize) {
        let spans = values
            .into_iter()
            .map(|v| text_span(self, v))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let result = (spans.as_ptr(), spans.len());
        self.spans.push(spans);
        result
    }
}
fn integer(value: Option<i32>) -> TuiOptionalI32 {
    TuiOptionalI32 {
        present: value.is_some(),
