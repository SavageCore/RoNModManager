import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, realpathSync } from "node:fs";
import { pathToFileURL } from "node:url";

// git-cliff drops every subject that is not `type: description` (see
// packaging/flatpak/cliff-appstream.toml and cliff.toml), so such commits
// vanish from the AppStream changelog and the release notes without failing
// anything. Catch them at commit time and in CI instead.

export const TYPES = [
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

export const HEADER_MAX_LENGTH = 100;

/** @typedef {{ label: string, subject: string, errors: string[] }} Failure */
/** @typedef {{ log: (...args: unknown[]) => void, error: (...args: unknown[]) => void }} Io */

const CONVENTIONAL = /^([a-z]+)(?:\(([^)\s]+)\))?(!)?:(.*)$/;

// Subjects git, npm or the release flow write that are not changelog entries.
const EXEMPT = [
  /^Merge /,
  /^Revert "/,
  /^(?:fixup|squash|amend)! /,
  /^\d+\.\d+\.\d+$/,
];

/** @param {string} message */
export function subjectOf(message) {
  const line = message
    .split("\n")
    .find((candidate) => candidate.trim() !== "" && !candidate.startsWith("#"));
  return (line ?? "").trim();
}

/** @param {string} message */
export function checkSubject(message) {
  const subject = subjectOf(message);
  if (subject === "") return [];
  if (EXEMPT.some((pattern) => pattern.test(subject))) return [];

  const match = CONVENTIONAL.exec(subject);
  if (!match) {
    return [
      'Not "type(scope): description" - e.g. "fix: stop opening devtools by default"',
    ];
  }

  const [, type, , , rest] = match;
  const errors = [];
  const description = rest.trim();
  if (!TYPES.includes(type)) {
    errors.push(`Unknown type "${type}" - use one of: ${TYPES.join(", ")}`);
  }
  if (description === "") {
    errors.push("Empty description after the colon");
  } else if (!rest.startsWith(" ")) {
    errors.push('Add a space after the colon - e.g. "fix: stop doing that"');
  }
  if (subject.length > HEADER_MAX_LENGTH) {
    errors.push(
      `Subject is ${subject.length} chars - keep it within ${HEADER_MAX_LENGTH}`,
    );
  }
  if (description.endsWith(".")) {
    errors.push("Drop the trailing full stop");
  }
  return errors;
}

/** @param {string} range */
function resolveRange(range) {
  const [rawFrom, rawTo] = range.includes("..")
    ? range.split("..")
    : [null, range];
  const allZero = /^0+$/;
  return {
    from: rawFrom && !allZero.test(rawFrom) ? rawFrom : null,
    to: rawTo && !allZero.test(rawTo) ? rawTo : "HEAD",
  };
}

/** @param {string} range @param {string} cwd */
export function commitsInRange(range, cwd = process.cwd()) {
  const { from, to } = resolveRange(range);
  const args = ["log", "--format=%H%x1f%B%x00"];
  if (from) args.push(`${from}..${to}`);
  else args.push("-1", to);

  const output = execFileSync("git", args, {
    cwd,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });

  return output
    .split("\x00")
    .map((record) => record.trim())
    .filter((record) => record !== "")
    .map((record) => {
      const [hash, ...rest] = record.split("\x1f");
      return { hash, message: rest.join("\x1f") };
    });
}

/** @param {string} range @param {string} cwd */
export function checkRange(range, cwd = process.cwd()) {
  return commitsInRange(range, cwd)
    .map(({ hash, message }) => ({
      label: hash.slice(0, 7),
      subject: subjectOf(message),
      errors: checkSubject(message),
    }))
    .filter((commit) => commit.errors.length > 0);
}

/** @param {Failure[]} failures @param {Io} io */
function report(failures, io) {
  if (failures.length === 0) return 0;
  io.error(
    `\ncheck-commit-msg: ${failures.length} commit message(s) would be dropped from the changelog\n`,
  );
  for (const failure of failures) {
    io.error(`  ${failure.label}: ${failure.subject}`);
    for (const error of failure.errors) io.error(`    - ${error}`);
  }
  io.error(
    `\nTypes: ${TYPES.join(", ")}. Merge commits, fixup!/squash! and version bumps are exempt.`,
  );
  return 1;
}

/** @param {string[]} args @param {Io} io @param {string} cwd */
export function main(args, io = console, cwd = process.cwd()) {
  const failures = [];
  let handled = false;

  for (let index = 0; index < args.length; index += 1) {
    const flag = args[index];
    const value = args[index + 1];

    if (flag === "--range") {
      handled = true;
      if (value === undefined) {
        io.error("check-commit-msg: --range needs a git range");
        return 2;
      }
      failures.push(...checkRange(value, cwd));
      index += 1;
    } else if (flag === "--message") {
      handled = true;
      failures.push({
        label: "message",
        subject: subjectOf(value ?? ""),
        errors: checkSubject(value ?? ""),
      });
      index += 1;
    } else if (flag === "--help" || flag === "-h") {
      io.log(
        "usage: check-commit-msg.js <commit-msg-file> | --message <text> | --range <A..B>",
      );
      return 0;
    } else if (existsSync(flag)) {
      handled = true;
      const message = readFileSync(flag, "utf8");
      failures.push({
        label: flag,
        subject: subjectOf(message),
        errors: checkSubject(message),
      });
    }
  }

  if (!handled) {
    io.error("check-commit-msg: nothing to check");
    return 2;
  }
  return report(
    failures.filter((failure) => failure.errors.length > 0),
    io,
  );
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(realpathSync(process.argv[1])).href
) {
  process.exitCode = main(process.argv.slice(2));
}
