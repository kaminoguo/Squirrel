"""
Squirrel Memory Service entry point.

Run with: python -m sqrl [command]
"""

import argparse
import asyncio
import logging
import os
import signal
import sys

import structlog


def setup_logging(verbose: bool = False) -> None:
    """Configure structured logging."""
    level = logging.DEBUG if verbose else logging.INFO

    structlog.configure(
        processors=[
            structlog.stdlib.filter_by_level,
            structlog.stdlib.add_logger_name,
            structlog.stdlib.add_log_level,
            structlog.stdlib.PositionalArgumentsFormatter(),
            structlog.processors.TimeStamper(fmt="iso"),
            structlog.processors.StackInfoRenderer(),
            structlog.processors.format_exc_info,
            structlog.processors.UnicodeDecoder(),
            structlog.dev.ConsoleRenderer()
            if sys.stderr.isatty()
            else structlog.processors.JSONRenderer(),
        ],
        wrapper_class=structlog.stdlib.BoundLogger,
        context_class=dict,
        logger_factory=structlog.stdlib.LoggerFactory(),
        cache_logger_on_first_use=True,
    )

    logging.basicConfig(
        format="%(message)s",
        level=level,
        stream=sys.stderr,
    )


async def run_server(socket_path: str) -> None:
    """Run the IPC server."""
    from sqrl.ipc.handlers import create_handlers
    from sqrl.ipc.server import IPCServer

    log = structlog.get_logger()

    # Create handlers
    handlers = create_handlers()

    # Create server
    server = IPCServer(socket_path)
    for method, handler in handlers.items():
        server.register(method, handler)

    # Handle shutdown signals
    shutdown_event = asyncio.Event()

    def handle_signal(sig: int) -> None:
        log.info("shutdown_signal_received", signal=sig)
        shutdown_event.set()

    loop = asyncio.get_running_loop()
    for sig in (signal.SIGINT, signal.SIGTERM):
        loop.add_signal_handler(sig, handle_signal, sig)

    # Start server
    await server.start()
    log.info("server_started", socket_path=socket_path)

    # Wait for shutdown
    try:
        await shutdown_event.wait()
    except asyncio.CancelledError:
        pass
    finally:
        log.info("server_stopping")
        await server.stop()
        log.info("server_stopped")


def cmd_serve(args: argparse.Namespace) -> int:
    """Run the Memory Service IPC server."""
    setup_logging(args.verbose)
    log = structlog.get_logger()

    # Check required env vars
    if not os.getenv("SQRL_STRONG_MODEL"):
        log.error(
            "missing_env_var",
            var="SQRL_STRONG_MODEL",
            hint="Set to LiteLLM model ID, e.g., 'openrouter/anthropic/claude-3.5-sonnet'",
        )
        return 1

    log.info(
        "starting_memory_service",
        socket_path=args.socket,
        model=os.getenv("SQRL_STRONG_MODEL"),
    )

    try:
        asyncio.run(run_server(args.socket))
        return 0
    except KeyboardInterrupt:
        return 0
    except Exception as e:
        log.error("server_error", error=str(e))
        return 1


def cmd_version(args: argparse.Namespace) -> int:
    """Print version."""
    from sqrl import __version__

    print(f"sqrl {__version__}")
    return 0


async def run_extract(episode_file: str) -> int:
    """Run extraction on an episode file."""
    import json
    from pathlib import Path

    from sqrl.ipc.handlers import handle_process_episode

    log = structlog.get_logger()

    # Load episode
    episode_path = Path(episode_file)
    if not episode_path.exists():
        log.error("file_not_found", path=episode_file)
        return 1

    with open(episode_path) as f:
        data = json.load(f)

    log.info("processing_episode", file=episode_file, events=len(data.get("events", [])))

    result = await handle_process_episode({
        "project_id": data.get("project_id", "unknown"),
        "project_root": data.get("project_root", str(Path.cwd())),
        "events": data.get("events", []),
        "existing_user_styles": [],
        "existing_project_memories": [],
    })

    if result.get("skipped"):
        log.info("episode_skipped", reason=result.get("skip_reason"))
    else:
        styles = result.get("user_styles", [])
        memories = result.get("project_memories", [])
        log.info(
            "extraction_complete",
            user_styles=len(styles),
            project_memories=len(memories),
        )

        if styles:
            print("\nUser Styles:")
            for s in styles:
                print(f"  - {s.get('text')}")

        if memories:
            print("\nProject Memories:")
            for m in memories:
                print(f"  - [{m.get('category')}] {m.get('text')}")

    return 0


def cmd_extract(args: argparse.Namespace) -> int:
    """Run extraction on an episode file."""
    setup_logging(args.verbose)
    return asyncio.run(run_extract(args.episode))


def cmd_status(args: argparse.Namespace) -> int:
    """Show current memories."""
    from pathlib import Path

    from sqrl.storage import ProjectMemoryStorage, UserStyleStorage

    # User styles
    user_storage = UserStyleStorage()
    user_styles = user_storage.get_all()

    print("=== User Styles ===")
    if user_styles:
        for s in user_styles:
            print(f"  [{s.use_count}x] {s.text}")
    else:
        print("  (none)")

    # Project memories (if in a project)
    project_root = args.project or str(Path.cwd())
    project_db = Path(project_root) / ".sqrl" / "memory.db"

    print(f"\n=== Project Memories ({project_root}) ===")
    if project_db.exists():
        project_storage = ProjectMemoryStorage(project_root)
        grouped = project_storage.get_grouped()
        if grouped:
            for category, memories in grouped.items():
                print(f"\n  ## {category}")
                for m in memories:
                    print(f"    [{m.use_count}x] {m.text}")
        else:
            print("  (none)")
    else:
        print("  (no project database)")

    return 0


def cmd_sync(args: argparse.Namespace) -> int:
    """Sync user styles to agent.md files."""
    from pathlib import Path

    from sqrl.agents import sync_user_styles

    setup_logging(args.verbose)
    log = structlog.get_logger()

    project_root = args.project or str(Path.cwd())
    results = sync_user_styles(project_root)

    for path, success in results.items():
        status = "synced" if success else "failed"
        log.info("sync_result", file=path, status=status)

    return 0


def main() -> int:
    """Main entry point."""
    parser = argparse.ArgumentParser(
        prog="sqrl",
        description="Squirrel Memory Service",
    )
    parser.add_argument(
        "-v", "--verbose",
        action="store_true",
        help="Enable verbose logging",
    )

    subparsers = parser.add_subparsers(dest="command", help="Commands")

    # serve command
    serve_parser = subparsers.add_parser(
        "serve",
        help="Run the IPC server",
    )
    serve_parser.add_argument(
        "--socket",
        default="/tmp/sqrl_agent.sock",
        help="Unix socket path (default: /tmp/sqrl_agent.sock)",
    )
    serve_parser.set_defaults(func=cmd_serve)

    # version command
    version_parser = subparsers.add_parser(
        "version",
        help="Print version",
    )
    version_parser.set_defaults(func=cmd_version)

    # extract command
    extract_parser = subparsers.add_parser(
        "extract",
        help="Run extraction on an episode file",
    )
    extract_parser.add_argument(
        "episode",
        help="Path to episode JSON file",
    )
    extract_parser.set_defaults(func=cmd_extract)

    # status command
    status_parser = subparsers.add_parser(
        "status",
        help="Show current memories",
    )
    status_parser.add_argument(
        "--project",
        help="Project root path (default: current directory)",
    )
    status_parser.set_defaults(func=cmd_status)

    # sync command
    sync_parser = subparsers.add_parser(
        "sync",
        help="Sync user styles to agent.md files",
    )
    sync_parser.add_argument(
        "--project",
        help="Project root path (default: current directory)",
    )
    sync_parser.set_defaults(func=cmd_sync)

    args = parser.parse_args()

    if args.command is None:
        # Default to serve
        args.command = "serve"
        args.socket = "/tmp/sqrl_agent.sock"
        args.func = cmd_serve

    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
