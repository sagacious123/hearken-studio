// Runs the Tauri CLI with this script's arguments, whatever Node binary runs it. The CLI's own
// launcher takes the program's path for an argument unless the program is named "node", and in
// some VS Code terminals and runners it's VS Code's "Code Helper (Plugin)" running as Node.
const cli = require("@tauri-apps/cli/main");

cli.run(process.argv.slice(2), "npm run tauri").catch((err) => {
  cli.logError(err.message);
  process.exit(1);
});
