import { execSync } from "child_process";
import { readFileSync, writeFileSync } from "fs";

const version = JSON.parse(readFileSync("package.json", "utf8")).version;

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

console.log(
  `Synced version ${version} → tauri.conf.json, Cargo.toml, Cargo.lock`,
);
