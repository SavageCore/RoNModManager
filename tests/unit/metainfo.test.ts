/// <reference types="node" />
import { afterEach, describe, expect, it } from "vitest";
import { execFileSync } from "node:child_process";
import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const script = resolve("scripts/sync-version.js");
const config = resolve("packaging/flatpak/cliff-appstream.toml");
const metainfoPath =
  "packaging/flatpak/uk.savagecore.ronmodmanager.metainfo.xml";
const directories: string[] = [];

function run(directory: string, command: string, args: string[]) {
  return execFileSync(command, args, {
    cwd: directory,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
}

function fixture() {
  const directory = mkdtempSync(join(tmpdir(), "ronmm-version-"));
  directories.push(directory);
  mkdirSync(join(directory, "src-tauri/src"), { recursive: true });
  mkdirSync(join(directory, "packaging/flatpak"), { recursive: true });
  copyFileSync(
    config,
    join(directory, "packaging/flatpak/cliff-appstream.toml"),
  );
  writeFileSync(
    join(directory, "package.json"),
    JSON.stringify({ version: "0.0.16" }),
  );
  writeFileSync(
    join(directory, "src-tauri/tauri.conf.json"),
    JSON.stringify({
      version: "0.0.16",
      bundle: { targets: ["nsis", "deb", "appimage", "rpm"] },
    }),
  );
  writeFileSync(
    join(directory, "src-tauri/Cargo.toml"),
    '[package]\nname = "version-test"\nversion = "0.0.16"\nedition = "2021"\n',
  );
  writeFileSync(join(directory, "src-tauri/src/main.rs"), "fn main() {}\n");
  writeFileSync(
    join(directory, metainfoPath),
    "<component>\n  <id>test</id>\n  <releases>\n  </releases>\n</component>\n",
  );
  run(directory, "git", ["init", "-q"]);
  run(directory, "git", ["config", "user.name", "Version Test"]);
  run(directory, "git", ["config", "user.email", "test@example.invalid"]);
  commit(directory, "feat: original release");
  run(directory, "git", ["tag", "v0.0.16"]);
  return directory;
}

function commit(directory: string, message: string) {
  run(directory, "git", ["add", "."]);
  run(directory, "git", [
    "-c",
    "core.hooksPath=/dev/null",
    "commit",
    "--allow-empty",
    "-qm",
    message,
  ]);
}

function sync(directory: string) {
  run(directory, process.execPath, [script]);
  return readFileSync(join(directory, metainfoPath), "utf8");
}

function releases(xml: string) {
  const document = new DOMParser().parseFromString(xml, "application/xml");
  expect(document.querySelector("parsererror")).toBeNull();
  expect(document.querySelectorAll("releases")).toHaveLength(1);
  return [...document.querySelectorAll("release")].map((release) =>
    release.getAttribute("version"),
  );
}

afterEach(() => {
  for (const directory of directories.splice(0))
    rmSync(directory, { recursive: true, force: true });
});

describe("version hook AppStream generation", () => {
  it("generates a pre-tag release and preserves history without duplicates", () => {
    const directory = fixture();
    commit(directory, "fix: upcoming change & XML escaping");
    writeFileSync(
      join(directory, "package.json"),
      JSON.stringify({ version: "0.0.17" }),
    );
    const xml = sync(directory);
    expect(releases(xml)).toEqual(["0.0.17", "0.0.16"]);
    expect(xml).toContain("Upcoming change &amp; XML escaping");
    expect(xml).toContain("<id>test</id>");
    expect(sync(directory)).toBe(xml);

    const conf = join(directory, "src-tauri/tauri.conf.json");
    expect(readFileSync(conf, "utf8")).toContain(
      '"targets": ["nsis", "deb", "appimage", "rpm"]',
    );
    expect(() =>
      run(directory, resolve("node_modules/.bin/oxfmt"), [
        "--config",
        resolve(".oxfmtrc.json"),
        "--check",
        conf,
      ]),
    ).not.toThrow();
    expect(run(directory, "git", ["tag", "--list", "v0.0.17"]).trim()).toBe("");
    expect(
      readFileSync(join(directory, "src-tauri/Cargo.lock"), "utf8"),
    ).toContain('version = "0.0.17"');
  }, 30000);

  it("reruns for an existing tag without relabelling later commits", () => {
    const directory = fixture();
    commit(directory, "fix: belongs to a future release");
    const xml = sync(directory);
    expect(releases(xml)).toEqual(["0.0.16"]);
    expect(xml).toContain("Original release");
    expect(xml).not.toContain("Belongs to a future release");
    expect(sync(directory)).toBe(xml);
    const generated = run(directory, "git", [
      "cliff",
      "--config",
      "packaging/flatpak/cliff-appstream.toml",
    ]).trimEnd();
    expect(xml).toContain(generated);
  }, 30000);
});
