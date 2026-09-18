import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { AiDevsVerification, loadEnv } from "../src/services/index.js";

const TASK = "negotiations";
const PORT = Number(process.env.PORT ?? 8787);
const DATA_DIR = dirname(fileURLToPath(import.meta.url));
const DATA_URL = "https://hub.ag3nts.org/dane/s03e04_csv";

const normalize = (value) => String(value ?? "")
  .normalize("NFD").replace(/[\u0300-\u036f]/gu, "").toLowerCase();

const parseCsv = (text) => text.trim().split(/\r?\n/u).slice(1).map((line) => {
  const comma = line.lastIndexOf(",");
  return [line.slice(0, comma).trim(), line.slice(comma + 1).trim()];
});

const loadKnowledge = async () => {
  const get = async (file) => {
    try { return await readFile(join(DATA_DIR, file), "utf8"); }
    catch {
      const response = await fetch(`${DATA_URL}/${file}`);
      if (!response.ok) throw new Error(`Unable to download ${file}: ${response.status}`);
      return response.text();
    }
  };
  const [citiesText, itemsText, connectionsText] = await Promise.all([
    get("cities.csv"), get("items.csv"), get("connections.csv")
  ]);
  const cities = new Map(parseCsv(citiesText));
  const items = parseCsv(itemsText).map(([name, code]) => ({
    name, code, words: normalize(name).split(/[^a-z0-9]+/u).filter(Boolean)
  }));
  const offers = new Map();
  for (const [itemCode, cityCode] of parseCsv(connectionsText)) {
    if (!offers.has(itemCode)) offers.set(itemCode, []);
    offers.get(itemCode).push(cities.get(cityCode) ?? cityCode);
  }
  return { items, offers };
};

const chooseItem = (query, knowledge) => {
  const text = normalize(query);
  const byCode = knowledge.items.find((item) => text.includes(normalize(item.code)));
  if (byCode) return byCode;
  const queryWords = new Set(text.split(/[^a-z0-9]+/u).filter((word) => word.length > 1));
  return knowledge.items
    .map((item) => ({ item, score: item.words.reduce((score, word) => score + (queryWords.has(word) ? 1 : 0), 0) }))
    .filter(({ score }) => score > 0)
    .sort((left, right) => right.score - left.score || left.item.words.length - right.item.words.length)[0]?.item;
};

const sendJson = (response, status, body) => {
  response.writeHead(status, {
    "Content-Type": "application/json; charset=utf-8",
    "Access-Control-Allow-Origin": "*"
  });
  response.end(JSON.stringify(body));
};

const main = async () => {
  const knowledge = await loadKnowledge();
  const server = createServer(async (request, response) => {
    if (request.method === "OPTIONS") return sendJson(response, 200, {});
    if (request.method !== "POST" || request.url !== "/api/search") {
      return sendJson(response, 404, { output: "Not found" });
    }
    try {
      let body = "";
      for await (const chunk of request) body += chunk;
      const payload = JSON.parse(body || "{}");
      const item = chooseItem(payload.params, knowledge);
      if (!item) return sendJson(response, 200, {
        output: "Nie rozpoznano przedmiotu. Podaj nazwę, model lub kod przedmiotu."
      });
      const cities = knowledge.offers.get(item.code) ?? [];
      const output = `${item.name}: ${cities.length ? cities.join(", ") : "brak miast"}`;
      return sendJson(response, 200, { output: output.slice(0, 490) });
    } catch (error) {
      return sendJson(response, 400, { output: `Błąd zapytania: ${error.message}`.slice(0, 490) });
    }
  });
  server.listen(PORT, () => console.log(`Negotiations tool listening on port ${PORT}`));

  if (process.env.TOOL_URL && process.env.DEVS_KEY) {
    loadEnv();
    const verifier = AiDevsVerification.fromEnv();
    const result = await verifier.verify(TASK, { tools: [{
      URL: `${process.env.TOOL_URL}/api/search`,
      description: "Search the CSV inventory. Send one natural-language description of one needed item in params; the response lists every city selling it. Repeat for each item and intersect the returned city lists."
    }] });
    console.log(JSON.stringify(result, null, 2));
  }
};

main().catch((error) => { console.error(`Error: ${error.message}`); process.exitCode = 1; });
