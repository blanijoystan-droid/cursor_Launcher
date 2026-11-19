import { spawn } from "child_process";
import path from "path";
import os from "os";

const cargoBin = path.join(os.homedir(), ".cargo", "bin");
const env = { ...process.env };

const currentPath = env.PATH || env.Path || "";
env.PATH = `${cargoBin};${currentPath}`;
env.Path = `${cargoBin};${currentPath}`;

const child = spawn(
  process.platform === "win32" ? "cmd.exe" : "npx",
  process.platform === "win32"
    ? ["/c", "npx", "tauri", ...process.argv.slice(2)]
    : ["tauri", ...process.argv.slice(2)],
  {
    stdio: "inherit",
    env,
  }
);

child.on("exit", (code) => {
  process.exit(code ?? 0);
});
