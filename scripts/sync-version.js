import { execFileSync, execSync } from "child_process";
import { readFileSync, writeFileSync } from "fs";

const version = JSON.parse(readFileSync("package.json", "utf8")).version;
const tag = `v${version}`;
const existingTag = execFileSync("git", ["tag", "--list", tag], {
  encoding: "utf8",
}).trim();
const releases = execFileSync(
  "git",
  [
    "cliff",
    "--config",
    "packaging/flatpak/cliff-appstream.toml",
    ...(existingTag ? [] : ["--tag", tag]),
  ],
  { encoding: "utf8" },
).trimEnd();
const metainfoPath =
  "packaging/flatpak/uk.savagecore.ronmodmanager.metainfo.xml";
const metainfo = readFileSync(metainfoPath, "utf8");
const releasesPattern = /  <releases>[\s\S]*?  <\/releases>/;
if (!releasesPattern.test(metainfo) || !releasesPattern.test(releases)) {
  throw new Error("Missing AppStream releases block");
}

const tauriConf = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));
tauriConf.version = version;
writeFileSync(
  "src-tauri/tauri.conf.json",
  JSON.stringify(tauriConf, null, 2) + "\n",
);

const cargo = readFileSync("src-tauri/Cargo.toml", "utf8");
writeFileSync(
  "src-tauri/Cargo.toml",
  cargo.replace(/^version = ".*"/m, `version = "${version}"`),
);

execSync(
  "cargo metadata --format-version 1 --manifest-path src-tauri/Cargo.toml",
  { stdio: "ignore" },
);

writeFileSync(
  metainfoPath,
  metainfo.replace(releasesPattern, () => releases),
);

console.log(
  `Synced version ${version} → tauri.conf.json, Cargo.toml, Cargo.lock, metainfo.xml`,
);
