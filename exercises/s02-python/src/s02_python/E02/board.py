import json
import os
import time
from pathlib import Path

import requests

from s02_python.E02.tiles import format_board_for_model
from s02_python.services import AiDevsVerification

TASK_NAME = "electricity"
HUB_DATA_BASE = "https://hub.ag3nts.org/data"


class ElectricityBoard:
    def __init__(
        self,
        verification: AiDevsVerification,
        snapshots_dir: Path | None = None,
    ) -> None:
        self.verification = verification
        self.api_key = verification.api_key
        self.snapshots_dir = snapshots_dir or Path(__file__).resolve().parent / "snapshots"
        self.snapshots_dir.mkdir(parents=True, exist_ok=True)
        self.step = 0

    @property
    def png_url(self) -> str:
        return f"{HUB_DATA_BASE}/{self.api_key}/electricity.png"

    @property
    def json_url(self) -> str:
        return f"{HUB_DATA_BASE}/{self.api_key}/electricity.json"

    def reset(self) -> Path:
        resp = requests.get(f"{self.png_url}?reset=1")
        resp.raise_for_status()
        time.sleep(0.4)
        self.step = 0
        return self.save_snapshot("initial")

    def sync_from_hub(self) -> Path:
        return self.save_snapshot("current")

    def fetch_json(self) -> list[list[int]]:
        resp = requests.get(self.json_url)
        resp.raise_for_status()
        body = resp.json()
        if not isinstance(body, list):
            raise ValueError(f"Invalid electricity.json: {body}")
        return body

    def fetch_png(self) -> bytes:
        resp = requests.get(self.png_url)
        resp.raise_for_status()
        return resp.content

    def save_snapshot(self, label: str) -> Path:
        png_path = self.snapshots_dir / f"step_{self.step:03d}_{label}.png"
        json_path = self.snapshots_dir / f"step_{self.step:03d}_{label}.json"

        board = self.fetch_json()
        png_path.write_bytes(self.fetch_png())

        json_path.write_text(
            json.dumps(
                {
                    "step": self.step,
                    "label": label,
                    "board": board,
                    "png_url": self.png_url,
                    "json_url": self.json_url,
                    "description": format_board_for_model(board),
                },
                indent=2,
                ensure_ascii=False,
            ),
            encoding="utf-8",
        )
        return png_path

    def rotate(self, cell: str) -> dict[str, object]:
        parts = cell.split("x")
        if len(parts) != 2 or not all(p.isdigit() and 1 <= int(p) <= 3 for p in parts):
            raise ValueError(f"Invalid cell id: {cell!r} (expected AxB, 1-3)")
        result = self.verification.verify(TASK_NAME, {"rotate": cell})
        time.sleep(0.35)
        self.step += 1
        self.save_snapshot(f"after_{cell}")
        return result


def should_reset_on_start() -> bool:
    return os.environ.get("ELECTRICITY_RESET", "").strip() in ("1", "true", "yes")
