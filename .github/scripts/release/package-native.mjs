import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { isDeepStrictEqual, parseArgs } from "node:util";
import { gzipSync, gunzipSync } from "node:zlib";
import { Header, Parser } from "tar";

export const MAX_MODULE_SIZE = 500 * 1024 * 1024;

export function releaseMetadata(root) {
  const { version, nativeTargets } = JSON.parse(fs.readFileSync(path.join(root, "bindings/js/package.json"), "utf8"));
  if (!/^\d+\.\d+\.\d+(?:-beta\.\d+)?$/.test(version)) {
    throw new Error(`Unsupported release version: ${version}`);
  }
  const revision = execFileSync("git", ["rev-parse", "HEAD"], { cwd: root, encoding: "utf8" }).trim();
  return { version, revision, targets: nativeTargets };
}

export function libraryName(target) {
  return target.includes("windows") ? "termprobe_ffi.dll"
    : target.includes("apple") ? "libtermprobe_ffi.dylib" : "libtermprobe_ffi.so";
}

export function sha256(data) {
  return createHash("sha256").update(data).digest("hex");
}

export function validateLibrary(data, target) {
  const arm = target.startsWith("aarch64-");
  let valid = false;
  if (target.includes("linux")) {
    valid = data.length >= 20 && data.subarray(0, 6).equals(Buffer.from("\x7fELF\x02\x01"))
      && data.readUInt16LE(16) === 3 && data.readUInt16LE(18) === (arm ? 183 : 62);
  } else if (target.includes("apple")) {
    valid = data.length >= 16 && data.readUInt32LE(0) === 0xfeedfacf
      && data.readUInt32LE(4) === (arm ? 0x100000c : 0x1000007) && data.readUInt32LE(12) === 6;
  } else if (data.length >= 64) {
    const offset = data.readUInt32LE(60);
    valid = data.length >= offset + 24 && data.subarray(0, 2).equals(Buffer.from("MZ"))
      && data.subarray(offset, offset + 4).equals(Buffer.from("PE\0\0"))
      && data.readUInt16LE(offset + 4) === 0x8664 && (data.readUInt16LE(offset + 22) & 0x2000) !== 0;
  }
  if (!valid) throw new Error(`Library format or architecture does not match ${target}`);
}

export function archiveBytes(entries) {
  const blocks = [];
  for (const [name, data] of [...entries].sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0)) {
    const header = new Header({
      path: name, size: data.length, mode: 0o644, uid: 0, gid: 0,
      mtime: new Date(0), type: "File", uname: "", gname: "",
    });
    if (header.encode()) throw new Error(`Archive path requires extended headers: ${name}`);
    blocks.push(header.block, data, Buffer.alloc((512 - data.length % 512) % 512));
  }
  blocks.push(Buffer.alloc(1024));
  return gzipSync(Buffer.concat(blocks));
}

export function writeArchive(filename, entries) {
  const data = archiveBytes(entries);
  fs.writeFileSync(filename, data);
  fs.writeFileSync(`${filename}.sha256`, `${sha256(data)}  ${path.basename(filename)}\n`);
}

export function nativeFilename(version, target) {
  return `termprobe-native-${version}-${target}.tar.gz`;
