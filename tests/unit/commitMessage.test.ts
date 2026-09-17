// @ts-nocheck - exercises untyped Node tooling (see scripts/check-commit-msg.js).
import { afterEach, describe, expect, it } from "vitest";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import {
  HEADER_MAX_LENGTH,
  TYPES,
  checkRange,
  checkSubject,
  commitsInRange,
  main,
  subjectOf,
} from "../../scripts/check-commit-msg.js";

const script = resolve("scripts/check-commit-msg.js");
const config = resolve("packaging/flatpak/cliff-appstream.toml");
const directories: string[] = [];

function run(directory: string, command: string, args: string[]) {
  return execFileSync(command, args, {
    cwd: directory,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
}

function cli(args: string[]) {
  const result = spawnSync(process.execPath, [script, ...args], {
    encoding: "utf8",
  });
  return {
    status: result.status,
    stdout: result.stdout,
    stderr: result.stderr,
  };
}

// main() writes through its io, so tests run it in-process and read the report
// back instead of spawning (child output is invisible to coverage).
function capture() {
  const lines: string[] = [];
  return {
    lines,
    io: {
      log: (line: string) => lines.push(line),
      error: (line: string) => lines.push(line),
    },
  };
}

function repository() {
  const directory = mkdtempSync(join(tmpdir(), "ronmm-commit-msg-"));
  directories.push(directory);
  run(directory, "git", ["init", "-q"]);
  run(directory, "git", ["config", "user.name", "Commit Test"]);
  run(directory, "git", ["config", "user.email", "test@example.invalid"]);
  return directory;
}

function commit(directory: string, message: string) {
  run(directory, "git", [
    "-c",
    "core.hooksPath=/dev/null",
    "commit",
    "--allow-empty",
    "-qm",
    message,
  ]);
}

afterEach(() => {
  for (const directory of directories.splice(0))
    rmSync(directory, { recursive: true, force: true });
});

describe("conventional commit subjects", () => {
  it("accepts every type the conventions allow, with scope and breaking marker", () => {
    for (const type of TYPES) {
      expect(checkSubject(`${type}: describe the change`)).toEqual([]);
      expect(checkSubject(`${type}(mods): describe the change`)).toEqual([]);
      expect(checkSubject(`${type}(mods)!: describe the change`)).toEqual([]);
    }
  });

  it("rejects subjects that git-cliff would drop", () => {
    expect(checkSubject("Run the first-launch setup inside the tour")).toEqual([
      'Not "type(scope): description" - e.g. "fix: stop opening devtools by default"',
    ]);
    expect(checkSubject("0.0.16")).toEqual([]);
    expect(
      checkSubject("Refresh the first-run setup screenshots"),
    ).toHaveLength(1);
  });

  it("rejects an unknown type and names the allowed ones", () => {
    const errors = checkSubject("wip: half-finished tour work");
    expect(errors).toHaveLength(1);
    expect(errors[0]).toContain('Unknown type "wip"');
    expect(errors[0]).toContain("feat");
  });

  it("rejects an empty description, a missing space, a trailing full stop and an over-long subject", () => {
    expect(checkSubject("fix: ")).toContain(
      "Empty description after the colon",
    );
    expect(checkSubject("fix:no space")).toContain(
      'Add a space after the colon - e.g. "fix: stop doing that"',
    );
    expect(checkSubject("fix: something.")).toContain(
      "Drop the trailing full stop",
    );
    const long = `fix: ${"x".repeat(HEADER_MAX_LENGTH)}`;
    expect(checkSubject(long)).toContain(
      `Subject is ${long.length} chars - keep it within ${HEADER_MAX_LENGTH}`,
    );
  });

  it("exempts merge, revert, autosquash and version bump subjects", () => {
    for (const subject of [
      "Merge pull request #76 from SavageCore/imgbot",
      'Revert "Add the first-launch guided tour"',
      "fixup! feat: add the tour",
      "squash! fix: stop opening devtools by default",
      "0.0.17",
    ]) {
      expect(checkSubject(subject)).toEqual([]);
    }
  });

  it("reads the subject past comment and blank lines and tolerates an empty message", () => {
    expect(subjectOf("\n# comment\n  fix: real subject  \n\nbody")).toBe(
      "fix: real subject",
    );
    expect(subjectOf("# only comments\n")).toBe("");
    expect(checkSubject("")).toEqual([]);
    expect(checkSubject("chore: bumps\n\nBREAKING CHANGE: nope")).toEqual([]);
  });
});

describe("command line", () => {
  it("passes a conventional message file, as the commit-msg hook calls it", () => {
    const file = join(repository(), "COMMIT_EDITMSG");
    writeFileSync(file, "fix: stop opening devtools by default\n");
    const { io, lines } = capture();
    expect(main([file], io)).toBe(0);
    expect(lines).toEqual([]);
  });

  it("fails a non-conventional message file with the reason", () => {
    const file = join(repository(), "COMMIT_EDITMSG");
    writeFileSync(file, "Add the first-launch guided tour\n");
    const { io, lines } = capture();
    expect(main([file], io)).toBe(1);
    const report = lines.join("\n");
    expect(report).toContain("1 commit message(s) would be dropped");
    expect(report).toContain('Not "type(scope): description"');
    expect(report).toContain("Types: build, chore, ci");
  });

  it("checks a literal message, prints help and refuses empty input", () => {
    expect(main(["--message", "feat: add the tour"], capture().io)).toBe(0);
    expect(main(["--message", "Add the tour"], capture().io)).toBe(1);
    const help = capture();
    expect(main(["--help"], help.io)).toBe(0);
    expect(help.lines.join("\n")).toContain("usage: check-commit-msg.js");
    expect(
      main(
        ["--message", "feat: x", "/nonexistent/COMMIT_EDITMSG"],
        capture().io,
      ),
    ).toBe(0);

    const empty = capture();
    expect(main([], empty.io)).toBe(2);
    expect(empty.lines.join("\n")).toContain("nothing to check");

    const missingRange = capture();
    expect(main(["--range"], missingRange.io)).toBe(2);
    expect(missingRange.lines.join("\n")).toContain(
      "--range needs a git range",
    );
  });

  it("exits non-zero as a real process, which is what fails the commit", () => {
    const result = cli(["--message", "Add the tour"]);
    expect(result.status).toBe(1);
    expect(result.stderr).toContain("would be dropped");
  });
});

describe("range checking", () => {
  it("reports only the commits the range introduces", () => {
    const directory = repository();
    commit(directory, "feat: original release");
    run(directory, "git", ["tag", "v0.0.16"]);
    commit(directory, "Run the first-launch setup inside the tour");
    commit(directory, "fix: sync AppStream on version bump");

    const failures = checkRange("v0.0.16..HEAD", directory);
    expect(failures).toHaveLength(1);
    expect(failures[0].subject).toBe(
      "Run the first-launch setup inside the tour",
    );
    expect(failures[0].label).toHaveLength(7);
    expect(commitsInRange("v0.0.16..HEAD", directory)).toHaveLength(2);
  });

  it("falls back to the tip commit when a push reports an all-zero before sha", () => {
    const directory = repository();
    commit(directory, "feat: original release");
    commit(directory, "0.0.17");
    const zero = "0".repeat(40);
    expect(commitsInRange(`${zero}..HEAD`, directory)).toHaveLength(1);
    expect(commitsInRange(`${zero}..`, directory)).toHaveLength(1);
    expect(commitsInRange("HEAD", directory)).toHaveLength(1);
    expect(main(["--range", `${zero}..HEAD`], capture().io, directory)).toBe(0);

    commit(directory, "Add the first-launch guided tour");
    expect(main(["--range", "HEAD"], capture().io, directory)).toBe(1);
    const { io, lines } = capture();
    expect(main(["--range", `${zero}..HEAD`], io, directory)).toBe(1);
    expect(lines.join("\n")).toContain("Add the first-launch guided tour");
  });
});

describe("git-cliff parity", () => {
  it("keeps every allowed type in the AppStream changelog", () => {
    const directory = repository();
    for (const type of TYPES) commit(directory, `${type}: ${type} change`);

    const changelog = run(directory, "git", [
      "cliff",
      "--config",
      config,
      "--tag",
      "v1.0.0",
    ]);
    for (const type of TYPES) {
      const subject = `${type} change`;
      expect(changelog).toContain(
        `<li>${subject[0].toUpperCase()}${subject.slice(1)}</li>`,
      );
    }
  }, 30000);

  it("would drop a subject the checker rejects", () => {
    const directory = repository();
    commit(directory, "Run the first-launch setup inside the tour");
    const changelog = run(directory, "git", [
      "cliff",
      "--config",
      config,
      "--tag",
      "v1.0.0",
    ]);
    expect(changelog).not.toContain("first-launch setup inside the tour");
  }, 30000);
});
