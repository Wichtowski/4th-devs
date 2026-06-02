import json
import os
from typing import Any

import requests

from s02_python.services import (
    AiDevsVerification,
    VerificationError,
    extract_response_text,
    extract_tool_calls,
)
from s02_python.services.llm_client import LLMClient, load_env, pick_model

ZMAIL_URL = "https://hub.ag3nts.org/api/zmail"
TASK = "mailbox"
MAX_ITERATIONS = 40

SYSTEM_PROMPT = """You investigate an operator's active mailbox via zmail API tools.

Goal: find three values and submit them with submit_answer:
- date: YYYY-MM-DD when the security department plans to attack the power plant
- password: employee system password (likely still in the mailbox)
- confirmation_code: exactly 36 chars, format SEC- + 32 alphanumeric chars

Context:
- Wiktor (unknown surname) reported on us; his email is from proton.me domain
- Search syntax is Gmail-like: from:, to:, subject:, OR, AND, quotes, -exclude
- Mailbox is live — new mail may arrive; retry searches if something is missing
- Always fetch full message bodies with mail_get_messages before concluding
- Search iteratively: help → broad search → read messages → narrow search
- Use submit_answer with any fields you have; hub feedback tells what is wrong or missing
- When hub returns a flag (FLG:), call finish with the flag text
"""

TOOLS: list[dict[str, Any]] = [
    {
        "type": "function",
        "name": "mail_help",
        "description": "List all zmail API actions and parameters.",
        "parameters": {"type": "object", "properties": {}, "additionalProperties": False},
    },
    {
        "type": "function",
        "name": "mail_search",
        "description": "Search mailbox. Returns metadata only (no body). Paginate with page.",
        "parameters": {
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Gmail-style search query"},
                "page": {"type": "integer", "minimum": 1},
                "per_page": {"type": "integer", "minimum": 5, "maximum": 20},
            },
            "required": ["query"],
            "additionalProperties": False,
        },
    },
    {
        "type": "function",
        "name": "mail_get_inbox",
        "description": "List inbox threads (metadata only).",
        "parameters": {
            "type": "object",
            "properties": {
                "page": {"type": "integer", "minimum": 1},
                "per_page": {"type": "integer", "minimum": 5, "maximum": 20},
            },
            "additionalProperties": False,
        },
    },
    {
        "type": "function",
        "name": "mail_get_thread",
        "description": "List message IDs in a thread (no body).",
        "parameters": {
            "type": "object",
            "properties": {
                "thread_id": {"type": "integer", "description": "Numeric thread ID"},
            },
            "required": ["thread_id"],
            "additionalProperties": False,
        },
    },
    {
        "type": "function",
        "name": "mail_get_messages",
        "description": "Fetch full message content by rowID and/or messageID.",
        "parameters": {
            "type": "object",
            "properties": {
                "ids": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "rowID or 32-char messageID hashes",
                }
            },
            "required": ["ids"],
            "additionalProperties": False,
        },
    },
    {
        "type": "function",
        "name": "submit_answer",
        "description": "Submit partial or full answer to hub /verify for task mailbox.",
        "parameters": {
            "type": "object",
            "properties": {
                "password": {"type": "string"},
                "date": {"type": "string", "description": "YYYY-MM-DD"},
                "confirmation_code": {"type": "string"},
            },
            "additionalProperties": False,
        },
    },
    {
        "type": "function",
        "name": "finish",
        "description": "End the task after flag was obtained.",
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


class ZmailClient:
    def __init__(self, api_key: str) -> None:
        self.api_key = api_key

    def call(self, payload: dict[str, Any]) -> dict[str, Any]:
        body = {"apikey": self.api_key, **payload}
        response = requests.post(ZMAIL_URL, json=body, timeout=60)
        try:
            data = response.json()
        except Exception:
            data = {"ok": False, "error": response.text}
        if not response.ok:
            data = {"ok": False, "http_status": response.status_code, **data}
        return data


class MailboxAgent:
    def __init__(self, api_key: str, llm: LLMClient, model: str) -> None:
        self.zmail = ZmailClient(api_key)
        self.verification = AiDevsVerification(api_key)
        self.llm = llm
        self.model = model
        self.known: dict[str, str] = {}
        self.finished_flag: str | None = None

    def run_tool(self, name: str, args: dict[str, Any]) -> str:
        if name == "mail_help":
            return json.dumps(self.zmail.call({"action": "help", "page": 1}), ensure_ascii=False)

        if name == "mail_search":
            payload: dict[str, Any] = {
                "action": "search",
                "query": args["query"],
                "page": args.get("page", 1),
            }
            if "per_page" in args:
                payload["perPage"] = args["per_page"]
            return json.dumps(self.zmail.call(payload), ensure_ascii=False)

        if name == "mail_get_inbox":
            payload = {"action": "getInbox", "page": args.get("page", 1)}
            if "per_page" in args:
                payload["perPage"] = args["per_page"]
            return json.dumps(self.zmail.call(payload), ensure_ascii=False)

        if name == "mail_get_thread":
            return json.dumps(
                self.zmail.call({"action": "getThread", "threadID": args["thread_id"]}),
                ensure_ascii=False,
            )

        if name == "mail_get_messages":
            return json.dumps(
                self.zmail.call({"action": "getMessages", "ids": args["ids"]}),
                ensure_ascii=False,
            )

        if name == "submit_answer":
            answer: dict[str, str] = {}
            for key in ("password", "date", "confirmation_code"):
                value = args.get(key)
                if isinstance(value, str) and value.strip():
                    answer[key] = value.strip()
                    self.known[key] = answer[key]
            if not answer:
                return json.dumps({"ok": False, "error": "No fields provided"})
            try:
                result = self.verification.verify(TASK, answer)
                text = json.dumps(result, ensure_ascii=False)
                message = str(result.get("message", ""))
                if "FLG:" in message:
                    self.finished_flag = message
                return text
            except VerificationError as error:
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
        messages: list[dict[str, Any]] = [
            {
                "role": "user",
                "content": (
                    "Start with mail_help, then search for Wiktor's proton.me mail and "
                    "hunt date, password, and SEC- confirmation code. Submit when ready."
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
                        "content": "Keep using tools until you submit_answer and get the flag.",
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
                preview = result if len(result) <= 400 else result[:400] + "..."
                print(f"[{step}] {name}({args}) -> {preview}")

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
        if self.known:
            print(f"Known so far: {json.dumps(self.known, ensure_ascii=False)}")
        return self.finished_flag


def run() -> None:
    load_env()
    api_key = os.environ.get("DEVS_KEY", "").strip()
    if not api_key:
        raise ValueError("Missing DEVS_KEY")

    model = pick_model(
        os.environ.get("MAILBOX_MODEL", "").strip() or "gpt-5.4-mini"
    )
    print(f"Model: {model}")

    llm = LLMClient.from_env(model)
    agent = MailboxAgent(api_key, llm, model)
    flag = agent.run()
    if flag:
        print(f"Flag: {flag}")
    else:
        print("No flag yet — run again (mailbox may have new mail)")


if __name__ == "__main__":
    run()
