import { build } from "bun";

const production = process.argv.includes("--production");
const watch = process.argv.includes("--watch");

async function main() {
  const result = await build({
    entrypoints: ["client/src/extension.ts"],
    outdir: "dist",
    target: "node",
    format: "cjs",
    minify: production,
    sourcemap: !production ? "inline" : false,
    external: ["vscode"],
    naming: {
      entry: "extension.js",
    },
    logLevel: "silent",
  });

  if (!result.success) {
    console.error("[ERROR] Build failed:");
    for (const error of result.logs) {
      if (error.level === "error") {
        console.error(`✘ [ERROR] ${error.message}`);
        if (error.position) {
          console.error(
            `    ${error.position.file}:${error.position.line}:${error.position.column}:`,
          );
        }
      }
    }
    process.exit(1);
  }

  console.log("[watch] build finished");
}

if (watch) {
  console.log("[watch] build started");
  main().catch((e) => {
    console.error(e);
    process.exit(1);
  });
} else {
  main().catch((e) => {
    console.error(e);
    process.exit(1);
  });
}