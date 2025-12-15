"""
Claude Code log parser.

Parses session logs from ~/.claude/projects/<project-hash>/*.jsonl
"""

import json
import re
from datetime import datetime
from pathlib import Path
from typing import Optional

from sqrl.parsers.base import (
    BaseParser,
    Episode,
    EpisodeStats,
    Event,
    EventKind,
    Frustration,
    Role,
)

# Frustration detection patterns (rule-based)
FRUSTRATION_PATTERNS = {
    Frustration.SEVERE: [
        r"\b(fuck|shit|damn|wtf|ffs)\b",
        r"!!{2,}",  # Multiple exclamation marks
    ],
    Frustration.MODERATE: [
        r"\b(finally|ugh|argh|sigh)\b",
        r"\b(why (won't|doesn't|isn't|can't))",
        r"\b(still (not|doesn't|won't))",
    ],
    Frustration.MILD: [
        r"\b(hmm|hm+)\b",
        r"\?{2,}",  # Multiple question marks
    ],
}

# Error detection patterns in tool results
ERROR_PATTERNS = [
    r"error:",
    r"exception:",
    r"traceback",
    r"failed",
    r"errno",
    r"permission denied",
    r"not found",
    r"syntax error",
]

# Max length for raw snippets (truncate longer content)
MAX_SNIPPET_LENGTH = 200

# Task boundary detection
TASK_BOUNDARY_GAP_MINUTES = 30  # Time gap indicating new task
MIN_EVENTS_PER_EPISODE = 3  # Minimum events to form an episode


def detect_frustration(text: str) -> Frustration:
    """Detect user frustration level from message text."""
    text_lower = text.lower()
    for level in [Frustration.SEVERE, Frustration.MODERATE, Frustration.MILD]:
        for pattern in FRUSTRATION_PATTERNS[level]:
            if re.search(pattern, text_lower, re.IGNORECASE):
                return level
    return Frustration.NONE


def is_error_result(text: str) -> bool:
    """Check if tool result indicates an error."""
    text_lower = text.lower()
    return any(re.search(p, text_lower) for p in ERROR_PATTERNS)


def truncate(text: str, max_len: int = MAX_SNIPPET_LENGTH) -> str:
    """Truncate text with ellipsis if too long."""
    if len(text) <= max_len:
        return text
    return text[: max_len - 3] + "..."


def summarize_content(content: list[dict]) -> tuple[str, Optional[str], Optional[str]]:
    """
    Extract summary, tool_name, and file from message content.

    Returns: (summary, tool_name, file_path)
    """
    summary_parts = []
    tool_name = None
    file_path = None

    for block in content:
        block_type = block.get("type", "")

        if block_type == "text":
            text = block.get("text", "")
            # Truncate long text for summary
            summary_parts.append(truncate(text, 100))

        elif block_type == "tool_use":
            tool_name = block.get("name")
            tool_input = block.get("input", {})
            # Extract file path if present
            if "file_path" in tool_input:
                file_path = tool_input["file_path"]
            elif "path" in tool_input:
                file_path = tool_input["path"]
            summary_parts.append(f"[{tool_name}]")

        elif block_type == "tool_result":
            result_content = block.get("content", "")
            if isinstance(result_content, str):
                summary_parts.append(truncate(result_content, 50))

    return " ".join(summary_parts) or "(empty)", tool_name, file_path


def detect_retry_loops(events: list[Event]) -> int:
    """
    Detect retry loops in events.

    A retry loop is when the same error occurs multiple times,
    indicating repeated failed attempts at the same task.
    """
    retry_count = 0
    recent_errors: list[str] = []

    for event in events:
        if event.is_error and event.summary:
            # Normalize error for comparison (first 50 chars)
            error_key = event.summary[:50].lower()

            # Check if similar error occurred recently
            for prev_error in recent_errors[-5:]:  # Look at last 5 errors
                if _similar_errors(error_key, prev_error):
                    retry_count += 1
                    break

            recent_errors.append(error_key)

    return retry_count


def _similar_errors(err1: str, err2: str) -> bool:
    """Check if two error strings are similar."""
    # Simple similarity: share significant words
    words1 = set(err1.split())
    words2 = set(err2.split())
    common = words1 & words2
    # Similar if they share more than 30% of words
    if not words1 or not words2:
        return False
    return len(common) / min(len(words1), len(words2)) > 0.3


def find_task_boundaries(events: list[Event]) -> list[int]:
    """
    Find indices where new tasks begin.

    Task boundaries are detected by:
    1. Large time gaps (> 30 minutes)
    2. User message after long assistant tool sequence

    Returns list of indices where new episodes should start.
    """
    if len(events) < 2:
        return [0]

    boundaries = [0]  # First event always starts an episode
    from datetime import timedelta

    gap_threshold = timedelta(minutes=TASK_BOUNDARY_GAP_MINUTES)

    for i in range(1, len(events)):
        prev_event = events[i - 1]
        curr_event = events[i]

        # Check time gap
        time_gap = curr_event.ts - prev_event.ts
        if time_gap > gap_threshold:
            # Large gap indicates new task
            boundaries.append(i)
            continue

        # Check for new user message after tool sequence
        if curr_event.role == Role.USER and curr_event.kind == EventKind.MESSAGE:
            # Count consecutive assistant events before this
            assistant_count = 0
            for j in range(i - 1, -1, -1):
                if events[j].role == Role.ASSISTANT:
                    assistant_count += 1
                else:
                    break

            # If there were many assistant actions, this might be a new task
            if assistant_count >= 10:
                boundaries.append(i)

    return boundaries


def split_into_episodes(
    events: list[Event],
    session_id: str,
    project_id: str,
) -> list[Episode]:
    """Split events into multiple episodes based on task boundaries."""
    if not events:
        return []

    boundaries = find_task_boundaries(events)

    # Add end marker
    boundaries.append(len(events))

    episodes = []
    for i in range(len(boundaries) - 1):
        start_idx = boundaries[i]
        end_idx = boundaries[i + 1]

        episode_events = events[start_idx:end_idx]

        # Skip tiny episodes (merge with previous if possible)
        if len(episode_events) < MIN_EVENTS_PER_EPISODE:
            if episodes:
                # Merge with previous episode
                prev_episode = episodes[-1]
                merged_events = prev_episode.events + episode_events
                episodes[-1] = Episode(
                    session_id=prev_episode.session_id,
                    project_id=prev_episode.project_id,
                    start_ts=prev_episode.start_ts,
                    end_ts=episode_events[-1].ts,
                    events=merged_events,
                    stats=_compute_stats(merged_events),
                )
            continue

        episode = Episode(
            session_id=f"{session_id}_{i}",
            project_id=project_id,
            start_ts=episode_events[0].ts,
            end_ts=episode_events[-1].ts,
            events=episode_events,
            stats=_compute_stats(episode_events),
        )
        episodes.append(episode)

    # If no episodes created, make one from all events
    if not episodes and events:
        episodes.append(
            Episode(
                session_id=session_id,
                project_id=project_id,
                start_ts=events[0].ts,
                end_ts=events[-1].ts,
                events=events,
                stats=_compute_stats(events),
            )
        )

    return episodes


def _compute_stats(events: list[Event]) -> EpisodeStats:
    """Compute episode statistics from events."""
    error_count = sum(1 for e in events if e.is_error)
    retry_loops = detect_retry_loops(events)

    # Find max frustration
    max_frustration = Frustration.NONE
    for event in events:
        if event.role == Role.USER and event.kind == EventKind.MESSAGE:
            frustration = detect_frustration(event.summary)
            if frustration.value > max_frustration.value:
                max_frustration = frustration

    return EpisodeStats(
        error_count=error_count,
        retry_loops=retry_loops,
        user_frustration=max_frustration,
    )


class ClaudeCodeParser(BaseParser):
    """Parser for Claude Code session logs."""

    def __init__(self, claude_dir: Optional[Path] = None):
        self.claude_dir = claude_dir or Path.home() / ".claude"

    def _get_project_hash(self, project_path: Path) -> str:
        """Convert project path to Claude Code's hash format."""
        # Claude Code uses path with / replaced by -, prefixed with -
        return "-" + str(project_path).replace("/", "-").lstrip("-")

    def get_sessions(self, project_path: Optional[Path] = None) -> list[Path]:
        """Find all session files for a project."""
        projects_dir = self.claude_dir / "projects"
        if not projects_dir.exists():
            return []

        if project_path:
            # Look for specific project
            project_hash = self._get_project_hash(project_path)
            project_dir = projects_dir / project_hash
            if not project_dir.exists():
                return []
            dirs = [project_dir]
        else:
            # All projects
            dirs = [d for d in projects_dir.iterdir() if d.is_dir()]

        sessions = []
        for d in dirs:
            # Get JSONL files, exclude agent-* files (sub-conversations)
            for f in d.glob("*.jsonl"):
                if not f.name.startswith("agent-"):
                    sessions.append(f)

        return sorted(sessions, key=lambda p: p.stat().st_mtime)

    def parse_session(self, session_path: Path) -> list[Episode]:
        """Parse a session file into episodes."""
        events: list[Event] = []
        session_id = session_path.stem
        project_id = session_path.parent.name

        with open(session_path) as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue

                try:
                    entry = json.loads(line)
                except json.JSONDecodeError:
                    continue

                entry_type = entry.get("type")
                if entry_type not in ("user", "assistant", "system"):
                    continue

                # Parse timestamp
                ts_str = entry.get("timestamp")
                if not ts_str:
                    continue
                try:
                    ts = datetime.fromisoformat(ts_str.replace("Z", "+00:00"))
                except ValueError:
                    continue

                # Get message content
                message = entry.get("message", {})
                content = message.get("content", [])
                if not content:
                    continue

                # Normalize content to list of dicts
                if isinstance(content, str):
                    content = [{"type": "text", "text": content}]
                elif not isinstance(content, list):
                    continue

                # Determine role
                role_str = message.get("role", entry_type)
                role = Role(role_str) if role_str in Role._value2member_map_ else Role.ASSISTANT

                # Determine event kind
                has_tool_use = any(b.get("type") == "tool_use" for b in content)
                has_tool_result = any(b.get("type") == "tool_result" for b in content)

                if has_tool_use:
                    kind = EventKind.TOOL_CALL
                elif has_tool_result:
                    kind = EventKind.TOOL_RESULT
                else:
                    kind = EventKind.MESSAGE

                # Extract summary
                summary, tool_name, file_path = summarize_content(content)

                # Check for errors in tool results
                is_error = False
                if kind == EventKind.TOOL_RESULT:
                    for block in content:
                        if block.get("type") == "tool_result":
                            result_text = str(block.get("content", ""))
                            if is_error_result(result_text):
                                is_error = True
                                break

                # Create event
                event = Event(
                    ts=ts,
                    role=role,
                    kind=kind,
                    summary=truncate(summary),
                    tool_name=tool_name,
                    file=file_path,
                    is_error=is_error,
                )
                events.append(event)

        if not events:
            return []

        # Split session into episodes based on task boundaries
        return split_into_episodes(events, session_id, project_id)
