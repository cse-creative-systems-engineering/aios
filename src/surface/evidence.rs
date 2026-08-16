//! Evidence index and value checks.
//!
//! `ToolResult` carries only a tool id and rendered text, so before the
//! composer runs we build an ordered `EvidenceIndex` that assigns each result
//! a stable key (`tool-0`, `tool-1`, ...). Widgets bind their values to those
//! keys via `SurfaceWidget::evidence`, and the validator uses
//! `value_present_in_evidence` / `number_present_in_evidence` to prove a
//! widget value actually came from a tool result. That proof is the
//! anti-hallucination guarantee for the surface: the model may select,
//! arrange, and copy, but it may not invent a measurement.

use crate::tools::ToolResult;

/// One indexed tool result, keyed for widget evidence bindings.
#[derive(Debug, Clone, PartialEq)]
pub struct EvidenceEntry {
    /// Stable key referenced by widget `evidence` bindings (e.g. `tool-0`).
    pub key: String,
    /// Tool id that produced the result (e.g. `power.observe_thermal`).
    pub tool: String,
    /// Rendered specialist output the composer may copy values from.
    pub text: String,
}

/// Ordered, keyed view of the tool results for one composition.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EvidenceIndex {
    entries: Vec<EvidenceEntry>,
}

impl EvidenceIndex {
    /// Build the index from the tool results returned by a chat outcome.
    pub fn from_results(results: &[ToolResult]) -> Self {
        Self {
            entries: results
                .iter()
                .enumerate()
                .map(|(index, result)| EvidenceEntry {
                    key: format!("tool-{index}"),
                    tool: result.tool.to_string(),
                    text: result.text.clone(),
                })
                .collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[EvidenceEntry] {
        &self.entries
    }

    /// Look up an entry by its evidence key.
    pub fn get(&self, key: &str) -> Option<&EvidenceEntry> {
        self.entries.iter().find(|entry| entry.key == key)
    }

    /// All evidence keys in order (`tool-0`, `tool-1`, ...).
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|entry| entry.key.as_str())
    }
}

/// Compose the evidence list exactly as the composer prompt should show it:
/// one `key (tool): text` line per result. The composer must quote keys back
/// verbatim in widget `evidence` bindings.
pub fn evidence_brief(index: &EvidenceIndex) -> String {
    index
        .entries
        .iter()
        .map(|entry| format!("{} ({}): {}", entry.key, entry.tool, entry.text))
        .collect::<Vec<_>>()
        .join("\n")
}

/// True when `value` appears verbatim in the entry text. Used for string
/// widget values (`MetricCard`, `StatusList`, `Notice`); the composer copies
/// the value exactly, so a substring match is the binding check.
pub fn value_present_in_evidence(entry: &EvidenceEntry, value: &str) -> bool {
    !value.is_empty() && entry.text.contains(value)
}

/// True when the numeric `value` appears in the entry text as a standalone
/// number rather than inside a longer digit run. This lets a widget extract
/// `63` from `temperature = 63C` while rejecting a match against `631` or a
/// different value with the same digits.
pub fn number_present_in_evidence(entry: &EvidenceEntry, value: f64) -> bool {
    text_contains_standalone_number(&entry.text, &format_number(value))
}

/// Render an f64 the way a specialist would quote it (`63`, `47.8`) so the
/// needle can be matched against the text literally.
fn format_number(value: f64) -> String {
    format!("{value}")
}

/// Search `text` for `needle` bounded by non-number characters (digit, dot,
/// minus) on both sides.
fn text_contains_standalone_number(text: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let text_bytes = text.as_bytes();
    let needle_len = needle.len();
    let mut search = 0;
    while let Some(relative) = text[search..].find(needle) {
        let start = search + relative;
        let end = start + needle_len;
        let before_ok = start == 0 || !is_number_char(text_bytes[start - 1]);
        let after_ok = end >= text_bytes.len() || !is_number_char(text_bytes[end]);
        if before_ok && after_ok {
            return true;
        }
        search = start + 1;
    }
    false
}

fn is_number_char(byte: u8) -> bool {
    byte.is_ascii_digit() || byte == b'.' || byte == b'-'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_result(tool: &'static str, text: &str) -> ToolResult {
        ToolResult {
            tool,
            text: text.to_string(),
        }
    }

    #[test]
    fn index_assigns_stable_keys_in_order() {
        let results = [
            tool_result("storage.observe_storage", "disk_used = 81%"),
            tool_result("power.observe_thermal", "temperature = 63C"),
        ];
        let index = EvidenceIndex::from_results(&results);
        assert_eq!(index.len(), 2);
        assert_eq!(index.keys().collect::<Vec<_>>(), vec!["tool-0", "tool-1"]);
        assert_eq!(index.get("tool-1").map(|e| e.tool.as_str()), Some("power.observe_thermal"));
        assert_eq!(index.get("tool-9"), None);
    }

    #[test]
    fn empty_results_give_empty_index() {
        let index = EvidenceIndex::from_results(&[]);
        assert!(index.is_empty());
    }

    #[test]
    fn evidence_brief_quotes_keys_and_tools() {
        let results = [tool_result("storage.observe_storage", "disk_used = 81%")];
        let index = EvidenceIndex::from_results(&results);
        let brief = evidence_brief(&index);
        assert_eq!(
            brief,
            "tool-0 (storage.observe_storage): disk_used = 81%"
        );
    }

    #[test]
    fn exact_copy_passes() {
        let index = EvidenceIndex::from_results(&[tool_result("storage.observe_storage", "healthy")]);
        let entry = index.get("tool-0").expect("entry");
        assert!(value_present_in_evidence(entry, "healthy"));
    }

    #[test]
    fn invented_value_fails() {
        let index = EvidenceIndex::from_results(&[tool_result("storage.observe_storage", "healthy")]);
        let entry = index.get("tool-0").expect("entry");
        assert!(!value_present_in_evidence(entry, "degraded"));
    }

    #[test]
    fn empty_value_never_matches() {
        let index = EvidenceIndex::from_results(&[tool_result("storage.observe_storage", "healthy")]);
        let entry = index.get("tool-0").expect("entry");
        assert!(!value_present_in_evidence(entry, ""));
    }

    #[test]
    fn numeric_extraction_from_text_passes() {
        let results = [
            tool_result("power.observe_thermal", "temperature = 63C"),
            tool_result("power.observe_thermal", "fan = 63.5 RPM"),
        ];
        let index = EvidenceIndex::from_results(&results);
        assert!(number_present_in_evidence(index.get("tool-0").expect("entry"), 63.0));
        assert!(number_present_in_evidence(index.get("tool-1").expect("entry"), 63.5));
    }

    #[test]
    fn number_inside_digit_run_fails() {
        // "63" is present in "631C" but is not the standalone value.
        let index = EvidenceIndex::from_results(&[tool_result(
            "power.observe_thermal",
            "temperature = 631C",
        )]);
        let entry = index.get("tool-0").expect("entry");
        assert!(!number_present_in_evidence(entry, 63.0));
    }

    #[test]
    fn negative_number_matches_with_sign() {
        let index = EvidenceIndex::from_results(&[tool_result(
            "power.observe_thermal",
            "sensor ambient = -10C",
        )]);
        let entry = index.get("tool-0").expect("entry");
        assert!(number_present_in_evidence(entry, -10.0));
        // "10" must not match "-10" as a standalone number.
        assert!(!number_present_in_evidence(entry, 10.0));
    }

    #[test]
    fn cross_tool_reference_fails() {
        // The value lives in tool-1's text; tool-0 must not satisfy it.
        let results = [
            tool_result("storage.observe_storage", "disk_used = 81%"),
            tool_result("power.observe_thermal", "temperature = 63C"),
        ];
        let index = EvidenceIndex::from_results(&results);
        let storage = index.get("tool-0").expect("storage entry");
        assert!(!number_present_in_evidence(storage, 63.0));
        assert!(!value_present_in_evidence(storage, "63C"));
        let thermal = index.get("tool-1").expect("thermal entry");
        assert!(number_present_in_evidence(thermal, 63.0));
    }

    #[test]
    fn percent_values_are_standalone_numbers() {
        let index = EvidenceIndex::from_results(&[tool_result(
            "storage.observe_storage",
            "disk_used = 81%",
        )]);
        let entry = index.get("tool-0").expect("entry");
        assert!(number_present_in_evidence(entry, 81.0));
        assert!(!number_present_in_evidence(entry, 8.0));
    }
}
