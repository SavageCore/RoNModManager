/// <reference types="node" />
import { describe, expect, it } from "vitest";
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import lint from "@commitlint/lint";
import load from "@commitlint/load";

// Same qualified rules the CLI uses, resolved from commitlint.config.js.
const { rules } = await load({}, { cwd: resolve(".") });
const TYPES = [
  "build",
  "chore",
  "ci",
  "docs",
  "feat",
  "fix",
  "perf",
  "refactor",
  "revert",
  "style",
  "test",
];

async function errorsFor(message: string) {
  const report = await lint(message, rules);
  return report.errors.map((error) => error.name);
}

// commitlint reports on stdout, so the helper returns both streams.
function editCommitMessage(message: string) {
  const directory = mkdtempSync(join(tmpdir(), "ronmm-commitlint-"));
  const file = join(directory, "COMMIT_EDITMSG");
  writeFileSync(file, message);
  try {
    const stdout = execFileSync(
      resolve("node_modules/.bin/commitlint"),
      ["--edit", file],
      {
        cwd: resolve("."),
        encoding: "utf8",
        stdio: ["ignore", "pipe", "pipe"],
      },
    );
    return { status: 0, output: String(stdout) };
  } catch (error) {
    const failure = error as { status?: number; stdout?: string };
    return { status: failure.status ?? 1, output: String(failure.stdout) };
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

describe("commitlint config", () => {
  it("accepts every conventional type, with a scope", async () => {
    for (const type of TYPES) {
      expect(await errorsFor(`${type}: describe the change`)).toEqual([]);
      expect(await errorsFor(`${type}(mods): describe the change`)).toEqual([]);
    }
    expect(
      await errorsFor("chore(deps): update dependency typescript to v6"),
    ).toEqual([]);
  });

  it("rejects the subjects git-cliff would silently drop", async () => {
    expect(
      await errorsFor("Run the first-launch setup inside the tour"),
    ).toEqual(["subject-empty", "type-empty"]);
  });

  it("rejects an unknown type, a full stop, a capitalised subject and a long header", async () => {
    expect(await errorsFor("wip: half-finished tour work")).toEqual([
      "type-enum",
    ]);
    expect(await errorsFor("fix: something.")).toEqual(["subject-full-stop"]);
    expect(await errorsFor("fix: Fix the thing")).toEqual(["subject-case"]);
    expect(await errorsFor(`fix: ${"y".repeat(100)}`)).toEqual([
      "header-max-length",
    ]);
  });

  it("leaves body and footer line lengths alone", async () => {
    expect(
      await errorsFor(`chore: bump the toolchain\n\n${"x".repeat(200)}`),
    ).toEqual([]);
    expect(
      await errorsFor("fix: keep footers valid\n\nBREAKING CHANGE: yes"),
    ).toEqual([]);
  });

  it("ignores merges, autosquash and bare version bumps", async () => {
    for (const message of [
      "Merge pull request #76 from SavageCore/imgbot",
      "Merge remote-tracking branch 'origin/main'",
      'Revert "Add the first-launch guided tour"',
      "fixup! feat: add the tour",
      "squash! fix: stop opening devtools by default",
      "0.0.17",
    ]) {
      expect(await errorsFor(message)).toEqual([]);
    }
  });
});

describe("commit-msg hook command", () => {
  it("resolves commitlint.config.js from a message file and fails a bad subject", () => {
    const rejected = editCommitMessage(
      "Run the first-launch setup inside the tour\n",
    );
    expect(rejected.status).toBe(1);
    expect(rejected.output).toContain("type-empty");

    const accepted = editCommitMessage(
      "feat(tour): run the setup inside the tour\n",
    );
    expect(accepted).toEqual({ status: 0, output: "" });
  });

  it("lets a bare version bump commit through", () => {
    expect(editCommitMessage("0.0.17\n").status).toBe(0);
  });
});
