import base64
import json
import os
from pathlib import Path
from typing import Any

from s02_python.services import (
    AiDevsVerification,
    VerificationError,
    extract_response_text,
    extract_tool_calls,
)
from s02_python.services.llm_client import LLMClient, load_env, pick_model
from s02_python.services.openai_wrapper import JsonSchemaFormat

TASK = "drone"
DESTINATION = "PWR6132PL"
MAX_ITERATIONS = 25
_DATA_DIR = Path(__file__).resolve().parent

DRONE_DOCS = """DRN-BMB7 drone API (key methods):

Location:
- setDestinationObject(ID) — flight target, format [A-Z]{3}[0-9]+[A-Z]{2}. Power plant: PWR6132PL
- set(x,y) — landing sector on target map. x=column, y=row, 1-indexed, top-left is (1,1)
- flyToLocation — starts flight; needs destination, sector, height, power, engine on

Engine / flight:
- set(engineON) / set(engineOFF)
- set(power) — e.g. set(100%)
- set(xm) — altitude 1m–100m, e.g. set(10m)

Mission goals (any order):
- set(destroy) — destroy target object (reported to System)
- set(return) — return to base after mission (REQUIRED or drone is lost)
- set(video), set(image)

Diagnostics / reset:
- selfCheck, getConfig, hardReset

Required before flyToLocation: destination, sector (x,y), engineON, power, fly height, mission goals including set(return).
Strategy: set destination to PWR6132PL (power plant) but sector coordinates to the DAM (bright cyan water), plus set(destroy).
"""

SYSTEM_PROMPT = f"""You program an armed drone for a sabotage mission.

{DRONE_DOCS}

You have map analysis with dam sector coordinates. Build minimal instruction list, submit via submit_instructions, read hub feedback, adjust and retry until flag (FLG:) appears.

Keep instructions lean — only what is needed. Use hardReset if config is corrupted from failed attempts.
"""

TOOLS: list[dict[str, Any]] = [
    {
        "type": "function",
        "name": "submit_instructions",
        "description": "Send drone instruction array to hub /verify for task drone.",
        "parameters": {
            "type": "object",
            "properties": {
                "instructions": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Drone API instructions in order",
                }
            },
            "required": ["instructions"],
            "additionalProperties": False,
        },
    },
    {
        "type": "function",
        "name": "finish",
        "description": "End after flag obtained.",
        "parameters": {
            "type": "object",
            "properties": {
                "flag": {"type": "string"},
                "summary": {"type": "string"},
            },
            "required": ["flag"],
            "additionalProperties": False,
        },
    },
]

DAM_SECTOR_SCHEMA = JsonSchemaFormat(
    "dam_sector",
    {
        "type": "object",
        "properties": {
            "grid_columns": {"type": "integer"},
            "grid_rows": {"type": "integer"},
            "dam_column": {"type": "integer"},
            "dam_row": {"type": "integer"},
            "notes": {"type": "string"},
        },
        "required": ["grid_columns", "grid_rows", "dam_column", "dam_row", "notes"],
        "additionalProperties": False,
    },
)


def map_image_url(api_key: str) -> str:
    return f"https://hub.ag3nts.org/data/{api_key}/drone.png"


def load_map_data_url(api_key: str) -> str:
    local = _DATA_DIR / "drone.png"
    if not local.is_file():
        import requests

        response = requests.get(map_image_url(api_key), timeout=60)
        response.raise_for_status()
        local.write_bytes(response.content)
    encoded = base64.b64encode(local.read_bytes()).decode()
    return f"data:image/png;base64,{encoded}"


def analyze_dam_sector(llm: LLMClient, map_data_url: str) -> dict[str, Any]:
    prompt = """Analyze this aerial map with a red grid overlay.
1. Count grid columns and rows (sectors between red lines).
2. Columns numbered left-to-right from 1; rows top-to-bottom from 1.
3. Find the DAM: horizontal concrete wall at the bottom with bright cyan/turquoise water (intensified color).
4. Return the sector (column, row) containing that bright water patch."""

    response = llm.responses(
        {
            "instructions": prompt,
            "input": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "input_text",
                            "text": "Locate dam sector. Grid 1-indexed, top-left is (1,1).",
                        },
                        {"type": "input_image", "image_url": map_data_url},
                    ],
                }
            ],
            "text": {
                "format": {
                    "type": "json_schema",
                    "name": DAM_SECTOR_SCHEMA.name,
                    "strict": DAM_SECTOR_SCHEMA.strict,
                    "schema": DAM_SECTOR_SCHEMA.schema,
                }
            },
        }
    )
    text = extract_response_text(response)
    if not text:
        raise RuntimeError(f"No vision response: {response}")
    return json.loads(text)


class DroneAgent:
    def __init__(
        self,
        api_key: str,
        llm: LLMClient,
        model: str,
        dam_sector: dict[str, Any],
    ) -> None:
        self.verification = AiDevsVerification(api_key)
        self.llm = llm
        self.model = model
        self.dam_sector = dam_sector
        self.finished_flag: str | None = None
        self.last_result: dict[str, Any] | None = None

    def run_tool(self, name: str, args: dict[str, Any]) -> str:
        if name == "submit_instructions":
            instructions = args.get("instructions", [])
            if not isinstance(instructions, list) or not instructions:
                return json.dumps({"ok": False, "error": "instructions must be non-empty array"})
            try:
                result = self.verification.verify(TASK, {"instructions": instructions})
                self.last_result = result
                message = str(result.get("message", ""))
                if "FLG:" in message:
                    self.finished_flag = message
                return json.dumps(result, ensure_ascii=False)
            except VerificationError as error:
                self.last_result = error.body
                return json.dumps(error.body, ensure_ascii=False)

        if name == "finish":
            self.finished_flag = args.get("flag", self.finished_flag or "")
            return json.dumps({"ok": True, "flag": self.finished_flag})

        return json.dumps({"ok": False, "error": f"Unknown tool: {name}"})

    def chat(self, messages: list[dict[str, Any]]) -> dict[str, Any]:
        return self.llm.responses(
            {
                "model": self.model,
                "instructions": SYSTEM_PROMPT,
                "input": messages,
                "tools": TOOLS,
                "tool_choice": "auto",
            }
        )

    def run(self) -> str | None:
        sector = self.dam_sector
        messages: list[dict[str, Any]] = [
            {
                "role": "user",
                "content": (
                    f"Map analysis: grid {sector['grid_columns']}x{sector['grid_rows']}, "
                    f"dam at column={sector['dam_column']}, row={sector['dam_row']}. "
                    f"Notes: {sector.get('notes', '')}. "
                    f"Target object code: {DESTINATION}. "
                    "Submit drone instructions to bomb the dam while reporting destroy on the power plant. "
                    "Iterate on hub errors until you get the flag."
                ),
            },
        ]

        for step in range(MAX_ITERATIONS):
            if self.finished_flag:
                print(f"\nDone: {self.finished_flag}")
                return self.finished_flag

            response = self.chat(messages)
            tool_calls = extract_tool_calls(response)

            if not tool_calls:
                content = extract_response_text(response) or ""
                print(f"[{step}] assistant: {content[:500]}")
                if "FLG:" in content:
                    self.finished_flag = content
                    return self.finished_flag
                messages.append(
                    {
                        "role": "user",
                        "content": "Use submit_instructions to send drone commands and get the flag.",
                    }
                )
                continue

            output = response.get("output")
            if isinstance(output, list):
                messages.extend(output)

            for tool_call in tool_calls:
                name = tool_call.get("name", "")
                raw_args = tool_call.get("arguments") or "{}"
                call_id = tool_call.get("call_id", "")
                try:
                    args = json.loads(raw_args)
                except json.JSONDecodeError:
                    args = {}

                result = self.run_tool(name, args)
                preview = result if len(result) <= 500 else result[:500] + "..."
                print(f"[{step}] {name}({json.dumps(args, ensure_ascii=False)[:200]}) -> {preview}")

                messages.append(
                    {
                        "type": "function_call_output",
                        "call_id": call_id,
                        "output": result,
                    }
                )

                if name == "finish" or (self.finished_flag and "FLG:" in self.finished_flag):
                    return self.finished_flag

        print("Max iterations reached")
        return self.finished_flag


def run() -> None:
    load_env()
    os.environ.setdefault("LLM_PROVIDER", "azure")

    api_key = os.environ.get("DEVS_KEY", "").strip()
    if not api_key:
        raise ValueError("Missing DEVS_KEY")

    model = pick_model(os.environ.get("DRONE_MODEL", "").strip() or "gpt-5.4")
    print(f"Model: {model} (provider: azure)")

    llm = LLMClient.from_env(model)
    if llm.provider != "azure":
        print(f"Warning: expected azure provider, got {llm.provider}")

    print("Analyzing map for dam sector...")
    map_data_url = load_map_data_url(api_key)
    dam_sector = analyze_dam_sector(llm, map_data_url)
    print(f"Dam sector: {json.dumps(dam_sector, ensure_ascii=False)}")

    agent = DroneAgent(api_key, llm, model, dam_sector)
    flag = agent.run()
    if flag:
        print(f"Flag: {flag}")
    else:
        print("No flag yet — run again")


if __name__ == "__main__":
    run()
