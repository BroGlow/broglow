// Keep the real plugin and every command it spawns in one sandbox HOME.
import childProcess from "node:child_process";
import { syncBuiltinESMExports } from "node:module";
import { setImmediate } from "node:timers/promises";
import { pathToFileURL } from "node:url";

const children = new Map();
const spawn = childProcess.spawn;
let stopping = false;
childProcess.spawn = (...args) => {
  if (stopping) throw new Error("test fixture is stopping");
  const child = spawn(...args);
  children.set(child, new Promise(resolve => child.once("close", () => {
    children.delete(child);
    resolve();
  })));
  return child;
};
syncBuiltinESMExports();

async function drain() {
  do {
    await Promise.all(children.values());
    // An exit can resolve the plugin's queue and schedule the next spawn.
    // Observe that turn too; this also works if sends become concurrent.
    await setImmediate();
  } while (children.size);
}

async function stop() {
  stopping = true;
  for (const child of children.keys()) child.kill("SIGKILL");
  await drain();
  process.exit(0);
}

const { broglow } = await import(pathToFileURL(process.env.PLUGIN_FILE));
const hooks = await broglow(JSON.parse(process.argv[2]));
process.on("message", async message => {
  if (message.kind === "stop") return stop();
  if (message.kind === "drain") await drain();
  else await hooks.event({ event: message.event });
  process.send("accepted");
});
process.on("disconnect", stop);
process.send("ready");
