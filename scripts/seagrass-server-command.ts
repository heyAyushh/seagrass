export function seagrassServerCommand(): [string, string[]] {
  if (process.env.SEAGRASS_SERVER_BINARY) {
    return [process.env.SEAGRASS_SERVER_BINARY, []];
  }
  if (process.env.SEAGRASS_HOTPATH === "1") {
    return [
      "cargo",
      ["run", "-p", "seagrass-cli", "--features", "hotpath", "--release", "--quiet"],
    ];
  }
  return ["cargo", ["run", "-p", "seagrass-cli", "--quiet"]];
}
