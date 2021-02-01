let sessionCounter = 0;

export function uniqueSession(prefix?: string): string {
  const sanitized = (prefix ?? "termprobe").replace(/[^A-Za-z0-9_-]/g, "-");
