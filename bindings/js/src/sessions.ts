import { resolveSession } from "./config.js";
import * as native from "./native.js";

export async function sessions(): Promise<string[]> {
  return native.sessions();
}

