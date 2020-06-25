import { fixture, attachment } from "./fixture.mjs";
import { parseReport } from "../src/model.js";
import { createResources } from "../src/resources.js";

if (!global.gc) throw new Error("The allocation probe requires --expose-gc");
async function retainedBytes() {
  global.gc();
  await new Promise(setImmediate);
  global.gc();
