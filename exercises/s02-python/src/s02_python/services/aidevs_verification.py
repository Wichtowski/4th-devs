import os
import requests
from typing import Any

VERIFY_URL = "https://hub.ag3nts.org/verify"

class VerificationError(Exception):
    """Raised when the hub returns a non-success response."""

    def __init__(self, status_code: int, body: dict[str, Any]):
        self.status_code = status_code
        self.body = body
        self.code = body.get("code", -1)
        self.message = body.get("message", "Unknown error")
        self.debug = body.get("debug", {})
        super().__init__(f"[{status_code}] ({self.code}) {self.message}")

class AiDevsVerification:
    def __init__(self, api_key: str):
        self.api_key = api_key

    @classmethod
    def from_env(cls) -> "AiDevsVerification":
        api_key = os.environ.get("DEVS_KEY", "").strip()
        if not api_key:
            raise ValueError("Missing or empty DEVS_KEY in environment")
        return cls(api_key)

    def verify(self, task: str, answer: Any) -> dict[str, Any]:
        if not task.strip():
            raise ValueError("Task cannot be empty")

        payload = {
            "apikey": self.api_key,
            "task": task,
            "answer": answer,
        }

        response = requests.post(VERIFY_URL, json=payload)

        try:
            body = response.json()
        except Exception:
            body = {"message": response.text}

        if not response.ok:
            raise VerificationError(response.status_code, body)

        return body