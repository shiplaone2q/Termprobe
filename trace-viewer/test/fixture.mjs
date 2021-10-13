import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

export const hostile = '</script><script>globalThis.reportPwned=true</script>\n<img src=x onerror="globalThis.reportPwned=true"> & $& /* REPORT_JS */ null /* REPORT_DATA */';
export const geometry = { grid_x: 7, grid_y: 11, cell_width: 10, cell_height: 20 };
export const query = (text, style = {}) => ({
  selector: { kind: "text", selector: { text, regex: false, full: false, whitespace: "exact", scope: {} } },
  occurrence: "any", direction: "within", style,
});
const blank = {
  char: " ", width: 1, fg: "default", bg: "default", underline_color: "default", underline_style: "none",
  resolved_fg: "#ffffff", resolved_bg: "#000000", resolved_underline_color: "#ffffff", flags: [],
};

function frame(sequence, cols, rows, text) {
  return {
    sequence, first_seen_ms: sequence * 10, last_seen_ms: sequence * 10 + 9, repeat_count: 1,
    size: { cols, rows }, cursor: { column: 3, row: 0, shape: "block", visible: true }, changes: ["text", "style"], text,
    cells: [
      blank, { ...blank, char: "A", fg: 1, resolved_fg: "#112233" }, { ...blank, char: "\u4f60", width: 2 },
      { ...blank, char: "", width: 0 }, { ...blank, char: "e\u0301", flags: ["bold"], link: "javascript:globalThis.reportPwned=true", link_id: "test" },
    ],
    grid: Array.from({ length: rows }, (_, row) => Array.from({ length: cols }, (_, col) => row === 0 && col < 4 ? col + 1 : 0)),
    svg: `<svg xmlns="http://www.w3.org/2000/svg" width="${cols * 10 + 14}" height="${rows * 20 + 22}" viewBox="0 0 ${cols * 10 + 14} ${rows * 20 + 22}"><rect width="100%" height="100%" fill="#000"/><text x="7" y="26" fill="#112233" font-family="monospace">A\u4f60e\u0301</text></svg>`,
  };
}

function operation(sequence, name, screen, expectation, result = "ok", input) {
  return {
    sequence, name, screen_before: Math.max(0, screen - 1), screen_at_return: screen,
