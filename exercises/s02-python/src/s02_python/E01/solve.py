import json

import requests

from s02_python.services import AiDevsVerification, CsvService, VerificationError

TASK_NAME = "categorize"

PROMPT_PREFIX = (
"Answer DNG or NEU.\n"
"DNG: explosive, weapon, toxic chemical, biohazard.\n"
"NEU: everything else.\n"
"Reactor-related items are always NEU.\n"
"Item:"
)

# Secret order: J-D-I-B-A-C-G-E-H-F (1-indexed letters A=1st row, B=2nd, ...)
SECRET_ORDER = "JDIBACGEHF"

def letter_to_index(letter: str) -> int:
    """A=0, B=1, ..., J=9"""
    return ord(letter) - ord("A")

def reorder_items(items: list[dict]) -> list[dict]:
    return [items[letter_to_index(ch)] for ch in SECRET_ORDER]

def fetch_csv(api_key: str) -> str:
    url = f"https://hub.ag3nts.org/data/{api_key}/categorize.csv"
    resp = requests.get(url)
    resp.raise_for_status()
    return resp.text

def reset(verification: AiDevsVerification) -> dict:
    return verification.verify(TASK_NAME, {"prompt": "reset"})

def classify(verification: AiDevsVerification, code: str, description: str) -> dict:
    prompt = f"{PROMPT_PREFIX} {code} {description}"
    return verification.verify(TASK_NAME, {"prompt": prompt})

def pp(label: str, data: dict):
    print(f"\n[{label}]")
    print(json.dumps(data, indent=2, ensure_ascii=False))

def run():
    verification = AiDevsVerification.from_env()

    # ── Reset budget ──
    reset_result = reset(verification)
    pp("RESET", reset_result)

    # ── Fetch fresh CSV ──
    csv_text = fetch_csv(verification.api_key)
    items = CsvService.read_records(csv_text)

    print(f"\n[CSV] Original order ({len(items)} items):\n")
    for i, item in enumerate(items):
        letter = chr(ord("A") + i)
        print(f"  {letter} (row {i+1})  {item['code']:>8}  {item['description']}")

    # ── Reorder by secret sequence ──
    ordered = reorder_items(items)

    print(f"\n[SECRET ORDER] {SECRET_ORDER} → sending in this sequence:\n")
    for i, item in enumerate(ordered):
        letter = SECRET_ORDER[i]
        orig_idx = letter_to_index(letter)
        print(f"  {i+1:>2}. {letter} (was row {orig_idx+1})  {item['code']:>8}  {item['description']}")
    print()

    # ── Classify each item ──
    for i, item in enumerate(ordered, 1):
        code = item["code"]
        description = item["description"]

        try:
            result = classify(verification, code, description)
            pp(f"RESPONSE {i}/10 — {code}", result)

            output = result.get("debug", {}).get("output", "?")
            balance = result.get("debug", {}).get("balance", "?")
            cached = result.get("debug", {}).get("cached_tokens", 0)
            tokens = result.get("debug", {}).get("tokens", 0)

            print(
                f"  → {code}  =  {output:>3}"
                f"  (tokens={tokens}, cached={cached}, balance={balance})"
            )

            msg = result.get("message", "")
            if "FLG:" in msg:
                print(f"\n{'=' * 60}")
                print(f"  🏁 FLAG: {msg}")
                print(f"{'=' * 60}")
                return

        except VerificationError as e:
            pp(f"ERROR {i}/10 — {code}", e.body)
            print(f"\n[ABORT] Fix prompt and retry.")
            return

    print("\n[DONE] All 10 items classified — no flag returned.")
