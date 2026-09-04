// Reads the version out of the manifest vump keeps in step, so running this
// shows what a bump actually changed.
const { name, version } = require("./package.json");

console.log(`${name} ${version}`);
