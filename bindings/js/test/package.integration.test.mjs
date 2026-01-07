import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { join } from "node:path";
import { test } from "node:test";
import { pathToFileURL } from "node:url";

const packageRoot = process.env.TERMPROBE__TEST_PACKAGE_ROOT;

if (packageRoot) {
  test("packed binding works without the CLI", async () => {
    const requireFromPackage = createRequire(join(packageRoot, "package.json"));
    const packageEntry = requireFromPackage.resolve("@microsoft/termprobe");
    assert.equal(
      spawnSync("termprobe", ["--version"], { stdio: "ignore" }).error?.code,
      "ENOENT",
    );

    const { Termprobe } = await import(pathToFileURL(packageEntry).href);
    const session = Termprobe.ephemeral("release-integration");
    try {
      await session.open();
      await session.submit("echo release-integration");
      await session.waitCommand();
      await session.getByText("release-integration").first().expect();
    } finally {
      await session.closeQuiet();
    }
  });
}
