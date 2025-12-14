"""IPC method handlers for Squirrel Memory Service.

Implements:
- IPC-001: ingest_chunk
- IPC-002: embed_text
- IPC-003: compose_context (optional, v1 uses templates)
"""

from typing import Any

import structlog

from sqrl.embeddings import embed_text as generate_embedding

log = structlog.get_logger()

# JSON-RPC error codes (per specs/INTERFACES.md)
ERR_CHUNK_EMPTY = -32001
ERR_INVALID_PROJECT = -32002
ERR_LLM_ERROR = -32003
ERR_EMPTY_TASK = -32010
ERR_EMPTY_TEXT = -32040
ERR_EMBEDDING_ERROR = -32041


class JsonRpcError(Exception):
    """JSON-RPC error with code and message."""

    def __init__(self, code: int, message: str, data: Any = None):
        self.code = code
        self.message = message
        self.data = data
        super().__init__(message)


async def handle_embed_text(params: dict) -> dict:
    """IPC-002: Generate embedding for text.

    Args:
        params: {"text": "..."}

    Returns:
        {"embedding": [float, ...]}
    """
    text = params.get("text", "")

    if not text or not text.strip():
        raise JsonRpcError(ERR_EMPTY_TEXT, "Empty text")

    log.info("embed_text_request", text_length=len(text))

    try:
        embedding = generate_embedding(text)
        log.info("embed_text_success", dimension=len(embedding))
        return {"embedding": embedding}
    except ValueError as e:
        raise JsonRpcError(ERR_EMPTY_TEXT, str(e))
    except Exception as e:
        log.error("embed_text_error", error=str(e))
        raise JsonRpcError(ERR_EMBEDDING_ERROR, f"Embedding error: {e}")


async def handle_ingest_chunk(params: dict) -> dict:
    """IPC-001: Process event chunk with Memory Writer.

    Args:
        params: {
            "project_id": "...",
            "owner_type": "user",
            "owner_id": "...",
            "chunk_index": 0,
            "events": [...],
            "carry_state": null,
            "recent_memories": [...]
        }

    Returns:
        {
            "episodes": [...],
            "memories": [...],
            "carry_state": null,
            "discard_reason": null
        }
    """
    project_id = params.get("project_id")
    events = params.get("events", [])
    chunk_index = params.get("chunk_index", 0)

    if not events:
        raise JsonRpcError(ERR_CHUNK_EMPTY, "Chunk empty")

    if not project_id:
        raise JsonRpcError(ERR_INVALID_PROJECT, "Invalid project")

    log.info(
        "ingest_chunk_request",
        project_id=project_id,
        chunk_index=chunk_index,
        event_count=len(events),
    )

    # TODO: Implement Memory Writer with LLM
    # For now, return empty response (no memories extracted)
    log.warning("memory_writer_not_implemented", message="Returning empty response")

    return {
        "episodes": [],
        "memories": [],
        "carry_state": None,
        "discard_reason": "Memory Writer not yet implemented",
    }


async def handle_compose_context(params: dict) -> dict:
    """IPC-003: Compose context from memories (optional).

    Args:
        params: {
            "task": "...",
            "memories": [...],
            "token_budget": 400
        }

    Returns:
        {
            "context_prompt": "...",
            "used_memory_ids": [...]
        }
    """
    task = params.get("task", "")
    memories = params.get("memories", [])

    if not task or not task.strip():
        raise JsonRpcError(ERR_EMPTY_TASK, "Empty task")

    log.info("compose_context_request", task_length=len(task), memory_count=len(memories))

    # v1: Use simple template (LLM composition is optional)
    lines = []
    used_ids = []

    for mem in memories:
        kind = mem.get("kind", "note")
        text = mem.get("text", "")
        mem_id = mem.get("id")

        if kind == "guard":
            lines.append(f"WARNING: {text}")
        elif kind == "invariant":
            lines.append(f"- {text}")
        else:
            lines.append(f"- {text}")

        if mem_id:
            used_ids.append(mem_id)

    context_prompt = "\n".join(lines)

    return {
        "context_prompt": context_prompt,
        "used_memory_ids": used_ids,
    }


# Method dispatch table
HANDLERS = {
    "embed_text": handle_embed_text,
    "ingest_chunk": handle_ingest_chunk,
    "compose_context": handle_compose_context,
}


async def dispatch(method: str, params: dict) -> Any:
    """Dispatch method to handler.

    Args:
        method: JSON-RPC method name.
        params: Method parameters.

    Returns:
        Method result.

    Raises:
        JsonRpcError: If method not found or handler fails.
    """
    handler = HANDLERS.get(method)
    if handler is None:
        raise JsonRpcError(-32601, f"Method not found: {method}")

    return await handler(params)
