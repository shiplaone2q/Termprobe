import type { Size, EffectiveTimeouts } from "../../bindings/js/src/types.js";

export type FailureReason =
  | "completed"
  | "test_failed"
  | "timed_out"
  | "session_exited"
  | "cancelled"
  | "locator_no_match"
  | "locator_ambiguous"
  | "unexpected_match"
  | "match_not_actionable"
  | "scalar_mismatch"
  | "snapshot_mismatch"
  | "emulator_fault"
  | "internal_failure";

export type LocatorFailureReason =
  | "anchor_not_found"
  | "anchor_ambiguous"
  | "relative_region_no_match"
  | "style_filter_removed_all"
  | "link_filter_removed_all"
  | "intersection_empty"
  | "union_empty"
  | "filter_removed_all"
  | "nth_out_of_range"
  | "outside_viewport"
  | "matched_no_cells"
  | "no_match"
  | "ambiguous";

export type FailureMatchOccurrence =
  | "any"
  | "unique"
  | "first"
  | "last"
  | { readonly nth: number };

export interface FailureTextPosition {
  readonly row: number;
  readonly column: number;
}

export interface FailureTextSpan {
  readonly row: number;
  readonly start: number;
  readonly end: number;
}

export interface FailureTextMatch {
  readonly text: string;
  readonly start: FailureTextPosition;
  readonly end: FailureTextPosition;
  readonly spans: readonly FailureTextSpan[];
}

export interface FailureTextStyle {
  readonly foreground?: string | null;
  readonly background?: string | null;
  readonly bold?: boolean | null;
  readonly dim?: boolean | null;
  readonly italic?: boolean | null;
  readonly underline_style?: string | null;
  readonly underline_color?: string | null;
  readonly inverse?: boolean | null;
  readonly hidden?: boolean | null;
  readonly strikethrough?: boolean | null;
  readonly blink?: boolean | null;
}

export interface FailureTextAnchor {
  readonly text: string;
  readonly regex: boolean;
  readonly occurrence: FailureMatchOccurrence;
}

export interface FailureTextSelector {
  readonly text: string;
  readonly regex: boolean;
  readonly full: boolean;
  readonly whitespace: "exact" | "normalize";
  readonly scope: {
    readonly after?: FailureTextAnchor | null;
    readonly before?: FailureTextAnchor | null;
  };
}

export interface FailureStyleSelector {
  readonly style: FailureTextStyle;
  readonly full: boolean;
}

export type FailureLocatorSelector =
  | { readonly kind: "text"; readonly selector: FailureTextSelector }
  | { readonly kind: "style"; readonly selector: FailureStyleSelector }
  | { readonly kind: "link"; readonly selector: { readonly uri: string; readonly full: boolean } }
  | { readonly kind: "and" | "or"; readonly selector: { readonly left: FailureLocatorQuery; readonly right: FailureLocatorQuery } }
  | { readonly kind: "filter"; readonly selector: { readonly input: FailureLocatorQuery; readonly has?: FailureLocatorQuery | null; readonly has_not?: FailureLocatorQuery | null } };

export interface FailureLocatorQuery {
  readonly selector: FailureLocatorSelector;
  readonly occurrence: FailureMatchOccurrence;
  readonly within?: FailureLocatorQuery | null;
  readonly direction: "within" | "after" | "before";
  readonly style: FailureTextStyle;
}

export type OperationExpectation =
  | { readonly kind: "locator"; readonly query: FailureLocatorQuery; readonly outcome: "matches" | "visible" | "hidden" | "unique" | "actionable" }
  | { readonly kind: "value"; readonly subject: string; readonly expected: string }
  | { readonly kind: "unavailable"; readonly reason: string };

export interface FailureOperationDetails {
  readonly name: string;
  readonly timeout_ms?: number;
  readonly elapsed_ms: number;
  readonly started_screen_sequence: number;
  readonly failed_screen_sequence: number;
}

export interface FailureCellMismatch {
  readonly location: FailureTextPosition;
  readonly grapheme: string;
  readonly property: string;
  readonly operator: string;
  readonly expected: string;
  readonly actual: string;
  readonly resolved?: string;
  readonly reason: string;
}

export interface FailureLocatorStageDetails {
  readonly stage_index: number;
  readonly expression_path: string;
  readonly evaluations: number;
  readonly failure_reason?: LocatorFailureReason;
  readonly mode: "text" | "contiguous_style_runs" | "parent_style_filter" | "contiguous_link_runs" | "parent_link_filter" | "intersection" | "union" | "containment_filter";
  readonly selector?: FailureLocatorSelector;
  readonly direction: "within" | "after" | "before";
  readonly requested_occurrence: FailureMatchOccurrence;
  /** Action operations may require a unique match. */
  readonly effective_occurrence: FailureMatchOccurrence;
  readonly occurrence_source: "explicit" | "action_default";
  readonly input_candidate_count: number;
  readonly raw_candidate_count: number;
  /** Candidates remaining after applying the requested styles. */
