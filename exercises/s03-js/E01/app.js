import { existsSync, mkdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { AiDevsVerification, loadEnv } from "../src/services/index.js";
import {
  classifyOperatorNote,
  downloadAndExtractZip,
  isMeasurementAnomaly,
  listJsonFiles,
  parseSensorRecord
} from "../src/evaluation/sensors.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const DATA_DIR = join(__dirname, ".cache", "sensors");
const ZIP_PATH = join(__dirname, ".cache", "sensors.zip");
const DATA_URL = "https://hub.ag3nts.org/dane/sensors.zip";
const TASK = "evaluation";

const ensureDataset = async () => {
  mkdirSync(join(__dirname, ".cache"), { recursive: true });
  await downloadAndExtractZip({
    url: DATA_URL,
    zipPath: ZIP_PATH,
    extractDir: DATA_DIR
  });
};

const main = async () => {
  loadEnv();
  await ensureDataset();

  const filePaths = listJsonFiles(DATA_DIR);
  const anomalies = [];

  for (const filePath of filePaths) {
    const record = parseSensorRecord(filePath);
    const measurementBad = isMeasurementAnomaly(record);
    const noteLabel = classifyOperatorNote(record.operator_notes);
    const noteBad = (noteLabel === "problem" && !measurementBad) || (noteLabel === "ok" && measurementBad);

    if (measurementBad || noteBad) {
      anomalies.push(record.id);
    }
  }

  anomalies.sort((left, right) => Number(left) - Number(right));

  console.log(`Found ${anomalies.length} anomalies`);
  console.log(JSON.stringify(anomalies));

  const verifier = AiDevsVerification.fromEnv();
  const result = await verifier.verify(TASK, { recheck: anomalies });
  console.log(JSON.stringify(result, null, 2));
};

main().catch((error) => {
  console.error(`Error: ${error.message}`);
  process.exit(1);
});
