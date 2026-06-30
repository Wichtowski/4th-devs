import { AiDevsVerification, HubShellClient, ShellError, loadEnv } from "../src/services/index.js";

const TASK = "firmware";
const BINARY = "/opt/firmware/cooler/cooler.bin";
const LOCKFILE = "/opt/firmware/cooler/cooler-is-blocked.lock";
const SETTINGS = "/opt/firmware/cooler/settings.ini";
const PASSWORD_NOTE = "/home/operator/notes/pass.txt";
const CONFIRMATION_PATTERN = /ECCS-[a-f0-9]{40}/i;

const shellSummary = (response) => {
  if (!response || typeof response !== "object") {
    return "";
  }

  const data = response.data;
  if (Array.isArray(data)) {
    return data.join("\n");
  }

  return typeof data === "string" ? data : "";
};

const firstMatchingPath = (paths, suffix) => paths.find((path) => path.endsWith(suffix)) ?? "";

const parseConfirmation = (text) => {
  const match = String(text).match(CONFIRMATION_PATTERN);
  if (!match) {
    throw new Error("Firmware output did not contain an ECCS confirmation code");
  }

  return match[0];
};

const prepareSettings = async (shell) => {
  try {
    await shell.rm(LOCKFILE);
  } catch (error) {
    if (
      !(error instanceof ShellError) ||
      (error.code !== 160 && error.code !== 410 && error.code !== -740)
    ) {
      throw error;
    }
  }

  await shell.editline(SETTINGS, 2, "SAFETY_CHECK=pass");
  await shell.editline(SETTINGS, 6, "enabled=false");
  await shell.editline(SETTINGS, 10, "enabled=true");
};

const discoverPassword = async (shell) => {
  const matches = await shell.find("*pass*");
  const paths = Array.isArray(matches.data) ? matches.data : [];
  const notePath = firstMatchingPath(paths, "/home/operator/notes/pass.txt") || PASSWORD_NOTE;

  const note = await shell.cat(notePath);
  const password = String(note.data ?? "").trim();
  if (!password) {
    throw new Error(`Password note was empty: ${notePath}`);
  }

  return password;
};

const runFirmware = async (shell, password) => {
  return shell.command(`${BINARY} ${password}`);
};

const main = async () => {
  loadEnv();
  const shell = HubShellClient.fromEnv({ retries: 4, timeoutMs: 60_000 });
  const verifier = AiDevsVerification.fromEnv();

  await prepareSettings(shell);
  const password = await discoverPassword(shell);
  const result = await runFirmware(shell, password);
  const confirmation = parseConfirmation(shellSummary(result));

  console.log(confirmation);

  const verification = await verifier.verify(TASK, { confirmation });
  console.log(JSON.stringify(verification, null, 2));
};

main().catch((error) => {
  console.error(`Error: ${error.message}`);
  process.exit(1);
});
