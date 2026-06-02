import os
from pathlib import Path
from typing import Any, Literal, Protocol

from dotenv import load_dotenv

from .azure_openai_wrapper import AzureOpenAiWrapper
from .openai_wrapper import JsonSchemaFormat, OpenAiWrapper

_EXERCISES_DIR = Path(__file__).resolve().parents[4]
_AVAILABLE_MODELS_PATH = _EXERCISES_DIR / "available_models"
_ENV_PATH = _EXERCISES_DIR / ".env"
DEFAULT_MODEL = "gpt-5.4-mini"
DEFAULT_AZURE_DEPLOYMENT = "gpt-5.4-nano"
_ENV_LOADED = False

Provider = Literal["openai", "azure"]


class CompletionClient(Protocol):
    def completion(
        self,
        model: str,
        system_prompt: str,
        user_prompt: str,
        json_schema_format: JsonSchemaFormat | None = None,
    ) -> str: ...


def load_env() -> None:
    global _ENV_LOADED
    if _ENV_LOADED:
        return
    if _ENV_PATH.is_file():
        load_dotenv(_ENV_PATH)
    _ENV_LOADED = True


def load_available_models() -> list[str]:
    if not _AVAILABLE_MODELS_PATH.is_file():
        return []
    lines = _AVAILABLE_MODELS_PATH.read_text(encoding="utf-8").splitlines()
    return [line.strip() for line in lines if line.strip()]


def pick_model(requested: str) -> str:
    available = load_available_models()
    if not available:
        return requested or DEFAULT_MODEL
    if requested in available:
        return requested
    if DEFAULT_MODEL in available:
        return DEFAULT_MODEL
    return available[0]


def resolve_azure_deployment(requested: str) -> str:
    load_env()
    for key in ("ELECTRICITY_AZURE_DEPLOYMENT", "OPENAI_AZURE_DEPLOYMENT"):
        explicit = os.environ.get(key, "").strip()
        if explicit:
            return explicit

    available = load_available_models()
    if requested in available:
        return requested
    if DEFAULT_AZURE_DEPLOYMENT in available:
        return DEFAULT_AZURE_DEPLOYMENT
    return available[0] if available else DEFAULT_AZURE_DEPLOYMENT


def resolve_provider(has_openai: bool, has_azure: bool) -> Provider:
    load_env()
    explicit = os.environ.get("LLM_PROVIDER", "").strip().lower()
    if explicit in ("openai", "azure"):
        if explicit == "openai" and not has_openai:
            raise ValueError("LLM_PROVIDER=openai but OPENAI_API_KEY is missing")
        if explicit == "azure" and not has_azure:
            raise ValueError(
                "LLM_PROVIDER=azure but Azure is not configured "
                "(OPENAI_AZURE_API_KEY + OPENAI_AZURE_ENDPOINT)"
            )
        return explicit

    if has_openai and not has_azure:
        return "openai"
    if has_azure and not has_openai:
        return "azure"
    return "openai"


def azure_endpoint_from_env() -> str:
    load_env()
    for key in (
        "OPENAI_AZURE_ENDPOINT",
        "AZURE_OPENAI_ENDPOINT",
        "OPENAI_AZURE_BASE_URL",
    ):
        value = os.environ.get(key, "").strip().rstrip("/")
        if value:
            return value
    return ""


def azure_config_from_env(model: str) -> dict[str, str] | None:
    load_env()
    api_key = os.environ.get("OPENAI_AZURE_API_KEY", "").strip()
    if not api_key:
        return None

    endpoint = azure_endpoint_from_env()
    if not endpoint:
        raise ValueError(
            "OPENAI_AZURE_API_KEY is set but endpoint is missing. "
            "Set OPENAI_AZURE_ENDPOINT in exercises/.env"
        )

    return {
        "api_key": api_key,
        "azure_endpoint": endpoint,
        "deployment": resolve_azure_deployment(model),
        "api_version": os.environ.get(
            "OPENAI_AZURE_API_VERSION", "2025-03-01-preview"
        ).strip(),
    }


class LLMClient:
    def __init__(
        self,
        provider: Provider,
        openai: OpenAiWrapper | None,
        azure: AzureOpenAiWrapper | None,
        model: str,
        azure_deployment: str,
    ) -> None:
        self.provider = provider
        self.openai = openai
        self.azure = azure
        self.model = model
        self.azure_deployment = azure_deployment

    @classmethod
    def from_env(cls, requested_model: str = "") -> "LLMClient":
        load_env()
        model = pick_model(
            os.environ.get("ELECTRICITY_MODEL", requested_model).strip()
            or requested_model
            or DEFAULT_MODEL
        )

        openai_client: OpenAiWrapper | None = None
        openai_key = os.environ.get("OPENAI_API_KEY", "").strip()
        if openai_key:
            openai_client = OpenAiWrapper(openai_key)

        azure_client: AzureOpenAiWrapper | None = None
        azure_cfg = azure_config_from_env(model)
        if azure_cfg is not None:
            azure_client = AzureOpenAiWrapper(
                api_key=azure_cfg["api_key"],
                azure_endpoint=azure_cfg["azure_endpoint"],
                deployment=azure_cfg["deployment"],
                api_version=azure_cfg["api_version"],
            )

        if openai_client is None and azure_client is None:
            raise ValueError(
                "Configure OPENAI_API_KEY and/or OPENAI_AZURE_API_KEY with OPENAI_AZURE_ENDPOINT"
            )

        provider = resolve_provider(
            openai_client is not None,
            azure_client is not None,
        )
        azure_deployment = resolve_azure_deployment(model)

        return cls(provider, openai_client, azure_client, model, azure_deployment)

    def responses(self, payload: dict[str, Any]) -> dict[str, Any]:
        body = dict(payload)
        body["model"] = pick_model(str(body.get("model") or self.model))

        if self.provider == "openai":
            if self.openai is None:
                raise RuntimeError("OpenAI client not configured")
            return self.openai.responses_raw(body)

        if self.azure is None:
            raise RuntimeError("Azure OpenAI client not configured")
        return self.azure.responses_raw(body)

    def completion(
        self,
        model: str,
        system_prompt: str,
        user_prompt: str,
        json_schema_format: JsonSchemaFormat | None = None,
    ) -> str:
        use_model = pick_model(model or self.model)

        if self.provider == "openai":
            if self.openai is None:
                raise RuntimeError("OpenAI client not configured")
            return self.openai.completion(
                use_model, system_prompt, user_prompt, json_schema_format
            )

        if self.azure is None:
            raise RuntimeError("Azure OpenAI client not configured")
        return self.azure.completion(
            use_model,
            system_prompt,
            user_prompt,
            json_schema_format,
        )


LlmClient = LLMClient
