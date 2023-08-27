import { fixture, attachment } from "./fixture.mjs";
import { parseReport } from "../src/model.js";
import { createResources } from "../src/resources.js";

if (!global.gc) throw new Error("The allocation probe requires --expose-gc");
async function retainedBytes() {
  global.gc();
  await new Promise(setImmediate);
  global.gc();
  return process.memoryUsage().arrayBuffers;
}
const data = fixture();
data.timeline.frames = [];
data.attachments = [attachment("session.cast", "x".repeat(64 * 1024 * 1024))];
const source = JSON.stringify(data);
const baseline = await retainedBytes();
