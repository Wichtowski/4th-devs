import os
import requests
from typing import Any

VERIFY_URL = "https://hub.ag3nts.org/verify"

class AiDevsVerification:
    def __init__(self, api_key: str):
        self.api_key = api_key

    @classmethod
    def from_env(cls) -> "AiDevsVerification":
        api_key = os.environ.get("DEVS_KEY", "").strip()
        if not api_key:
            raise ValueError("Missing or empty DEVS_KEY in environment")
        return cls(api_key)

    def verify(self, task: str, answer: Any) -> dict | str:
        if not task.strip():
            raise ValueError("Task cannot be empty")

        payload = {
            "apikey": self.api_key,
            "task": task,
            "answer": answer,
        }

        response = requests.post(VERIFY_URL, json=payload)

        if not response.ok:
            raise RuntimeError(f"Verify request failed ({response.status_code}): {response.text}")

        try:
            return response.json()
        except Exception:
            return response.text