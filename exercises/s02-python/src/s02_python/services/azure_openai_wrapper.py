import os
from typing import Any

from openai import AzureOpenAI

from .openai_wrapper import JsonSchemaFormat, extract_response_text

DEFAULT_API_VERSION = "2025-03-01-preview"


class AzureOpenAiWrapper:
    def __init__(
        self,
        api_key: str,
        azure_endpoint: str,
        deployment: str,
        api_version: str = DEFAULT_API_VERSION,
    ) -> None:
        self.deployment = deployment
        self._client = AzureOpenAI(
            api_key=api_key,
            azure_endpoint=azure_endpoint.rstrip("/"),
            api_version=api_version,
        )

    @classmethod
    def from_env(cls, deployment: str | None = None) -> "AzureOpenAiWrapper":
        api_key = os.environ.get("OPENAI_AZURE_API_KEY", "").strip()
        endpoint = os.environ.get("OPENAI_AZURE_ENDPOINT", "").strip()
        api_version = os.environ.get(
            "OPENAI_AZURE_API_VERSION", DEFAULT_API_VERSION
        ).strip()
        deploy = (
            deployment
            or os.environ.get("ELECTRICITY_AZURE_DEPLOYMENT", "").strip()
            or os.environ.get("OPENAI_AZURE_DEPLOYMENT", "").strip()
        )

        if not api_key:
            raise ValueError("Missing OPENAI_AZURE_API_KEY in environment")
        if not endpoint:
            raise ValueError("Missing OPENAI_AZURE_ENDPOINT in environment")
        if not deploy:
            raise ValueError(
                "Missing Azure deployment name (ELECTRICITY_AZURE_DEPLOYMENT or OPENAI_AZURE_DEPLOYMENT)"
            )

        return cls(api_key, endpoint, deploy, api_version)

    def responses_raw(self, payload: dict[str, Any]) -> dict[str, Any]:
        body = dict(payload)
        body.pop("model", None)
        response = self._client.responses.create(model=self.deployment, **body)
        return response.model_dump()

    def completion(
        self,
        model: str,
        system_prompt: str,
        user_prompt: str,
        json_schema_format: JsonSchemaFormat | None = None,
    ) -> str:
        del model
        payload = self._build_payload(system_prompt, user_prompt, json_schema_format)
        response = self._client.responses.create(
            model=self.deployment,
            **payload,
        )
        parsed = response.model_dump()
        text = extract_response_text(parsed)
        if text is None:
            raise RuntimeError(f"No text found in Azure response: {parsed}")
        return text

    @staticmethod
    def _build_payload(
        system_prompt: str,
        user_prompt: str,
        json_schema_format: JsonSchemaFormat | None,
    ) -> dict[str, Any]:
        payload: dict[str, Any] = {
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
