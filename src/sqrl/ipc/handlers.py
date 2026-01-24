"""IPC method handlers."""

from collections.abc import Awaitable, Callable
from typing import Any

import structlog

from sqrl.agents import MemoryExtractor, ProjectSummarizer, UserScanner
from sqrl.models import ProcessEpisodeRequest, ProcessEpisodeResponse
from sqrl.models.episode import EpisodeEvent
from sqrl.models.extraction import ProjectMemoryOp, UserStyleOp

log = structlog.get_logger()

Handler = Callable[[dict[str, Any]], Awaitable[Any]]


def _extract_user_messages(events: list[EpisodeEvent]) -> list[str]:
    """Extract just the user messages from events."""
    return [e.content_summary for e in events if e.role == "user"]


def _get_ai_turns(events: list[EpisodeEvent], trigger_index: int) -> str:
    """Get 3 AI turns before the trigger message.

    An AI turn = all events between two user messages.
    """
    # Find all user message indices
    user_indices = [i for i, e in enumerate(events) if e.role == "user"]

    if trigger_index >= len(user_indices):
        return ""

    # The actual event index of the trigger
    trigger_event_idx = user_indices[trigger_index]

    # Find start index for 3 AI turns before
    # Each AI turn is bounded by user messages
    # So we need to go back 3 user messages and collect everything in between
    start_turn = max(0, trigger_index - 3)
    start_event_idx = user_indices[start_turn] if start_turn > 0 else 0

    # Collect all events from start to trigger (excluding trigger itself)
    ai_turn_events = events[start_event_idx:trigger_event_idx]

    # Format as text
    return "\n".join(f"[{e.role}] {e.content_summary}" for e in ai_turn_events)


async def handle_process_episode(params: dict[str, Any]) -> dict[str, Any]:
    """IPC-001: process_episode handler.

    Session-end processing:
    1. Stage 1 (Flash): Scan all user messages, identify behavioral patterns
    2. Stage 2 (Pro): Extract memories from each flagged message
    3. Filter by confidence threshold (>0.8)
    """
    # Parse request
    try:
        request = ProcessEpisodeRequest(**params)
    except Exception as e:
        raise ValueError(f"Invalid params: {e}") from e

    log.info(
        "process_episode_start",
        project_id=request.project_id,
        events_count=len(request.events),
    )

    # Stage 1: User Scanner - scan all user messages for behavioral patterns
    user_messages = _extract_user_messages(request.events)
    if not user_messages:
        log.info("no_user_messages")
        return ProcessEpisodeResponse(
            skipped=True,
            skip_reason="No user messages in episode",
        ).model_dump()

    scanner = UserScanner()
    scanner_output = await scanner.scan(user_messages)

    if not scanner_output.has_patterns:
        log.info("no_patterns_detected")
        return ProcessEpisodeResponse(
            skipped=True,
            skip_reason="No behavioral patterns detected",
        ).model_dump()

    log.info("patterns_detected", indices=scanner_output.indices)

    # Stage 0: Get project summary for context
    summarizer = ProjectSummarizer()
    project_summary = await summarizer.summarize(request.project_root)
    if project_summary:
        log.info("project_summary_generated", length=len(project_summary))

    # Stage 2: Memory Extractor - process each flagged message
    all_user_styles: list[UserStyleOp] = []
    all_project_memories: list[ProjectMemoryOp] = []

    extractor = MemoryExtractor()

    for trigger_index in scanner_output.indices:
        if trigger_index >= len(user_messages):
            continue

        trigger_message = user_messages[trigger_index]
        ai_context = _get_ai_turns(request.events, trigger_index)

        extractor_output = await extractor.extract(
            project_id=request.project_id,
            project_root=request.project_root,
            trigger_message=trigger_message,
            ai_context=ai_context,
            existing_user_styles=request.existing_user_styles,
            existing_project_memories=request.existing_project_memories,
            project_summary=project_summary,
            apply_confidence_filter=True,  # Filter by confidence > 0.8
        )

        all_user_styles.extend(extractor_output.user_styles)
        all_project_memories.extend(extractor_output.project_memories)

    log.info(
        "process_episode_done",
        user_styles_count=len(all_user_styles),
        project_memories_count=len(all_project_memories),
    )

    return ProcessEpisodeResponse(
        skipped=False,
        user_styles=all_user_styles,
        project_memories=all_project_memories,
    ).model_dump()


def create_handlers() -> dict[str, Handler]:
    """Create all IPC handlers."""
    return {
        "process_episode": handle_process_episode,
    }
