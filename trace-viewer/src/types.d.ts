import type {
  FailureCellMismatch, FailureReport, FailureOperationEvent, FailureScreenSnapshot,
} from "./report.js";

export type Operation = FailureOperationEvent;
export interface FrameCell {
  readonly char: string;
  readonly width: number;
  readonly fg: string | number;
  readonly bg: string | number;
  readonly underline_color: string | number;
  readonly underline_style: string;
  readonly flags: readonly string[];
  readonly resolved_fg: string;
  readonly resolved_bg: string;
  readonly resolved_underline_color: string;
  readonly link?: string;
  readonly link_id?: string;
}
export interface Frame extends FailureScreenSnapshot {
  readonly cells: readonly FrameCell[];
  readonly grid: readonly (readonly number[])[];
  readonly svg?: string;
  readonly omission?: string;
}
export interface Geometry {
  readonly grid_x: number;
  readonly grid_y: number;
  readonly cell_width: number;
  readonly cell_height: number;
}
export interface Attachment {
  readonly name: string;
  readonly bytes: number;
  readonly data: string;
}
export interface ReportData {
  readonly details: FailureReport;
  readonly timeline: {
    readonly schema_version?: number;
    readonly frames: readonly Frame[];
    readonly geometry: Geometry;
    readonly failure_screen_sequence: number;
  };
  readonly explanation: { readonly title: string; readonly expected: string; readonly actual: string; readonly note: string };
  readonly files: readonly { readonly path: string; readonly status: string; readonly reason?: string }[];
  readonly errors: readonly string[];
  readonly attachments: readonly Attachment[];
}
