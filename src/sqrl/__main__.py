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

    args = parser.parse_args()

    if args.command is None:
        # Default to serve
        args.command = "serve"
        args.socket = "/tmp/sqrl_agent.sock"
        args.func = cmd_serve

    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
