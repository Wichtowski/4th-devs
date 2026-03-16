import os
from typing import Any

import requests

OPENAI_RESPONSES_URL = "https://api.openai.com/v1/responses"

class JsonSchemaFormat:
    def __init__(self, name: str, schema: dict[str, Any], strict: bool = True):
        self.name = name
        self.schema = schema
        self.strict = strict

class OpenAiWrapper:
    def __init__(
        self,
        api_key: str,
        endpoint: str = OPENAI_RESPONSES_URL,
        extra_headers: dict[str, str] | None = None,
    ):
        self.api_key = api_key
        self.endpoint = endpoint
        self.extra_headers = extra_headers or {}

    @classmethod
    def from_env(cls) -> "OpenAiWrapper":
        api_key = os.environ.get("OPENAI_API_KEY", "").strip()
        if not api_key:
            raise ValueError("Missing or empty OPENAI_API_KEY in environment")
        return cls(api_key)

    def completion(
        self,
        model: str,
        system_prompt: str,
        user_prompt: str,
        json_schema_format: JsonSchemaFormat | None = None,
    ) -> str:
        """High-level: build payload, send, return extracted text."""
        payload = self._build_payload(model, system_prompt, user_prompt, json_schema_format)
        response = self._post_json(self.endpoint, payload)
        parsed = self._parse_json_response(response)
        text = extract_response_text(parsed)

        if text is None:
            raise RuntimeError(f"No text found in response: {parsed}")
        return text

    def responses_raw(self, payload: dict[str, Any]) -> dict[str, Any]:
        """Low-level: send arbitrary payload, return raw parsed JSON."""
        resp = self._post_json(self.endpoint, payload)
        return self._parse_json_response(resp)

    @staticmethod
    def _build_payload(
        model: str,
        system_prompt: str,
        user_prompt: str,
        json_schema_format: JsonSchemaFormat | None = None,
    ) -> dict[str, Any]:
        payload: dict[str, Any] = {
            "model": model,
            "input": [
                {
                    "role": "system",
                    "content": [{"type": "input_text", "text": system_prompt}],
                },
                {
                    "role": "user",
                    "content": [{"type": "input_text", "text": user_prompt}],
                },
            ],
        }

        if json_schema_format is not None:
            payload["text"] = {
                "format": {
                    "type": "json_schema",
                    "name": json_schema_format.name,
                    "strict": json_schema_format.strict,
                    "schema": json_schema_format.schema,
                }
            }

        return payload

    def _post_json(self, url: str, payload: dict[str, Any]) -> requests.Response:
        headers = {
            "Content-Type": "application/json",
            "Authorization": f"Bearer {self.api_key}",
            **self.extra_headers,
        }
        return requests.post(url, json=payload, headers=headers)

    @staticmethod
    def _parse_json_response(response: requests.Response) -> dict[str, Any]:
        try:
            value = response.json()
        except Exception:
            raise RuntimeError(f"Response was not valid JSON: {response.text}")

        error_message = None
        if isinstance(value, dict) and "error" in value:
            error_obj = value["error"]
            if isinstance(error_obj, dict):
                error_message = error_obj.get("message")

        if not response.ok:
            raise RuntimeError(
                f"Request failed ({response.status_code}): {error_message or 'Unknown API error'}"
            )

        if error_message:
            raise RuntimeError(error_message)

        return value

def extract_response_text(response: dict[str, Any]) -> str | None:
    output_text = response.get("output_text")
    if isinstance(output_text, str) and output_text.strip():
        return output_text.strip()

    output = response.get("output")
    if not isinstance(output, list):
        return None

    for item in output:
        if not isinstance(item, dict) or item.get("type") != "message":
            continue
        contents = item.get("content")
        if not isinstance(contents, list):
            continue
        for content in contents:
            if isinstance(content, dict):
                text = content.get("text")
                if isinstance(text, str) and text.strip():
                    return text.strip()

    return None