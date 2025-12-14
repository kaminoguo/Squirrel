"""JSON-RPC 2.0 server over Unix socket for Squirrel Memory Service.

Socket path: /tmp/sqrl_agent.sock (per specs/INTERFACES.md)
"""

import asyncio
import json
import os
import signal
import sys
from pathlib import Path
from typing import Any

import structlog

from .handlers import JsonRpcError, dispatch

log = structlog.get_logger()

# Default socket path per ARCHITECTURE.md
DEFAULT_SOCKET_PATH = "/tmp/sqrl_agent.sock"


def make_response(id: Any, result: Any = None, error: dict | None = None) -> dict:
    """Create JSON-RPC 2.0 response."""
    response = {"jsonrpc": "2.0", "id": id}
    if error is not None:
        response["error"] = error
    else:
        response["result"] = result
    return response


def make_error(code: int, message: str, data: Any = None) -> dict:
    """Create JSON-RPC error object."""
    error = {"code": code, "message": message}
    if data is not None:
        error["data"] = data
    return error


async def handle_connection(reader: asyncio.StreamReader, writer: asyncio.StreamWriter):
    """Handle a single client connection."""
    addr = writer.get_extra_info("peername")
    log.info("client_connected", addr=addr)

    try:
        while True:
            # Read line-delimited JSON
            line = await reader.readline()
            if not line:
                break

            try:
                request = json.loads(line.decode("utf-8"))
            except json.JSONDecodeError as e:
                response = make_response(
                    None, error=make_error(-32700, f"Parse error: {e}")
                )
                writer.write(json.dumps(response).encode("utf-8") + b"\n")
                await writer.drain()
                continue

            # Extract request fields
            request_id = request.get("id")
            method = request.get("method", "")
            params = request.get("params", {})

            log.debug("request_received", method=method, id=request_id)

            # Dispatch to handler
            try:
                result = await dispatch(method, params)
                response = make_response(request_id, result=result)
            except JsonRpcError as e:
                response = make_response(
                    request_id, error=make_error(e.code, e.message, e.data)
                )
            except Exception as e:
                log.exception("handler_error", method=method)
                response = make_response(
                    request_id, error=make_error(-32603, f"Internal error: {e}")
                )

            # Send response
            writer.write(json.dumps(response).encode("utf-8") + b"\n")
            await writer.drain()

    except asyncio.CancelledError:
        pass
    except Exception:
        log.exception("connection_error")
    finally:
        log.info("client_disconnected", addr=addr)
        writer.close()
        await writer.wait_closed()


async def run_server(socket_path: str = DEFAULT_SOCKET_PATH):
    """Run the IPC server.

    Args:
        socket_path: Unix socket path.
    """
    # Remove existing socket file
    socket_file = Path(socket_path)
    if socket_file.exists():
        socket_file.unlink()

    # Start server
    server = await asyncio.start_unix_server(handle_connection, socket_path)

    log.info("server_started", socket_path=socket_path)

    # Handle shutdown signals
    loop = asyncio.get_running_loop()
    stop_event = asyncio.Event()

    def signal_handler():
        log.info("shutdown_signal_received")
        stop_event.set()

    for sig in (signal.SIGINT, signal.SIGTERM):
        loop.add_signal_handler(sig, signal_handler)

    try:
        async with server:
            await stop_event.wait()
    finally:
        log.info("server_stopped")
        # Clean up socket file
        if socket_file.exists():
            socket_file.unlink()


def configure_logging():
    """Configure structlog for the application."""
    structlog.configure(
        processors=[
            structlog.stdlib.add_log_level,
            structlog.processors.TimeStamper(fmt="iso"),
            structlog.processors.StackInfoRenderer(),
            structlog.processors.format_exc_info,
            structlog.dev.ConsoleRenderer(colors=True),
        ],
        wrapper_class=structlog.stdlib.BoundLogger,
        context_class=dict,
        logger_factory=structlog.PrintLoggerFactory(),
        cache_logger_on_first_use=True,
    )


def main():
    """Entry point for sqrl-agent command."""
    configure_logging()

    socket_path = os.environ.get("SQRL_SOCKET_PATH", DEFAULT_SOCKET_PATH)

    log.info(
        "starting_memory_service",
        version="0.1.0",
        socket_path=socket_path,
    )

    try:
        asyncio.run(run_server(socket_path))
    except KeyboardInterrupt:
        log.info("interrupted")
        sys.exit(0)


if __name__ == "__main__":
    main()
