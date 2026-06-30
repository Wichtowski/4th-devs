import { existsSync, mkdirSync, readdirSync, rmSync, readFileSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";
import { execFileSync } from "node:child_process";

export const SENSOR_ACTIVE_RANGES = {
  temperature: ["temperature_K", 553, 873],
  pressure: ["pressure_bar", 60, 160],
  water: ["water_level_meters", 5.0, 15.0],
  voltage: ["voltage_supply_v", 229.0, 231.0],
  humidity: ["humidity_percent", 40.0, 80.0]
};

export const POSITIVE_NOTE_PREFIXES = new Set([
  "Performance appears nominal",
  "Everything checks out",
  "This cycle looks reliable",
  "Readings are calm and predictable",
  "The operating profile stays normal",
  "No irregular behavior is visible",
  "Execution quality is high",
  "No concerning drift is present",
  "System behavior is fully stable",
  "Routine diagnostics are positive",
  "No warning signs appeared",
  "The recent snapshot is reassuring",
  "Current status remains healthy",
  "The trend line is quiet",
  "Health indicators remain strong",
  "This run finished without surprises",
  "Operational state is consistent",
  "The latest report looks clean",
  "The overall picture is solid",
  "Daily monitoring confirms stability",
  "Observed values stay controlled",
  "The process stayed balanced",
  "All telemetry looks steady",
  "Service condition is excellent",
  "Tracking data remains coherent",
  "The report looks completely normal"
]);

export const NEGATIVE_NOTE_PREFIXES = new Set([
  "The numbers feel inconsistent",
  "This state looks unstable",
  "The latest behavior is concerning",
  "These readings look suspicious",
  "The signal profile looks unusual",
  "This check did not look right",
  "This is not the pattern I expected",
  "I am seeing an unexpected pattern",
  "I can see a clear irregularity",
  "The current result seems unreliable",
  "The report does not look healthy",
  "There is a visible anomaly here",
  "This report raises serious doubts",
  "I am not comfortable with this result",
  "Something is clearly off",
  "The output quality is doubtful",
  "The situation requires attention",
  "This run shows questionable behavior"
]);

const openingClause = (note) => note.split(/[.,;]/u)[0].trim();

export const classifyOperatorNote = (note) => {
  const clause = openingClause(note);
  if (POSITIVE_NOTE_PREFIXES.has(clause)) {
    return "ok";
  }

  if (NEGATIVE_NOTE_PREFIXES.has(clause)) {
    return "problem";
  }

  const lower = clause.toLowerCase();
  if (
    lower.includes("suspicious") ||
    lower.includes("unstable") ||
    lower.includes("concerning") ||
    lower.includes("irregular") ||
    lower.includes("unexpected") ||
    lower.includes("inconsistent") ||
    lower.includes("unusual") ||
    lower.includes("doubtful") ||
    lower.includes("questionable") ||
    lower.includes("not look right") ||
    lower.includes("does not look healthy") ||
    lower.includes("anomaly") ||
    lower.includes("requires attention") ||
    /\boff\b/u.test(lower)
  ) {
    return "problem";
  }

  if (
    lower.includes("normal") ||
    lower.includes("healthy") ||
    lower.includes("stable") ||
    lower.includes("routine") ||
    lower.includes("clean") ||
    lower.includes("steady") ||
    lower.includes("reassuring") ||
    lower.includes("nominal") ||
    lower.includes("solid") ||
    lower.includes("balanced") ||
    lower.includes("controlled") ||
    lower.includes("predictable") ||
    lower.includes("no warning") ||
    lower.includes("no irregular") ||
    lower.includes("no concerning") ||
    lower.includes("no issue")
  ) {
    return "ok";
  }

  throw new Error(`Unknown operator note prefix: ${clause}`);
};

export const parseSensorRecord = (filePath) => {
  const data = JSON.parse(readFileSync(filePath, "utf8"));
  return {
    id: basename(filePath, ".json"),
    ...data
  };
};

export const isMeasurementAnomaly = (record) => {
  const activeSensors = new Set(record.sensor_type.split("/"));

  for (const [sensor, [field, min, max]] of Object.entries(SENSOR_ACTIVE_RANGES)) {
    const value = record[field];
    const isActive = activeSensors.has(sensor);

    if (isActive) {
      if (value < min || value > max) {
        return true;
      }
      continue;
    }

    if (value !== 0) {
      return true;
    }
  }

  return false;
};

export const listJsonFiles = (dir) => {
  const stack = [dir];
  const files = [];

  while (stack.length > 0) {
    const current = stack.pop();
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      const fullPath = join(current, entry.name);
      if (entry.isDirectory()) {
        stack.push(fullPath);
        continue;
      }

      if (entry.isFile() && entry.name.endsWith(".json")) {
        files.push(fullPath);
      }
    }
  }

  return files.sort();
};

export const downloadAndExtractZip = async ({ url, zipPath, extractDir, timeoutMs = 60_000 }) => {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);

  try {
    const response = await fetch(url, { signal: controller.signal });
    if (!response.ok) {
      throw new Error(`Failed to download dataset (${response.status})`);
    }

    const bytes = Buffer.from(await response.arrayBuffer());
    writeFileSync(zipPath, bytes);
  } finally {
    clearTimeout(timeout);
  }

  mkdirSync(extractDir, { recursive: true });
  const markerPath = join(extractDir, ".extracted");
  if (existsSync(markerPath)) {
    return;
  }

  rmSync(extractDir, { recursive: true, force: true });
  mkdirSync(extractDir, { recursive: true });

  try {
    execFileSync("unzip", ["-oq", zipPath, "-d", extractDir], {
      stdio: "inherit"
    });
  } catch {
    throw new Error("Failed to extract sensors.zip. Make sure the unzip command is available");
  }

  writeFileSync(markerPath, "ok\n");
};
