"""
IPC module for Squirrel.

JSON-RPC 2.0 server over Unix socket for communication
between Rust daemon and Python Memory Service.
"""

from sqrl.ipc.handlers import (
    EmbedTextHandler,
    EvaluateMemoriesHandler,
    IngestChunkHandler,
    SearchMemoriesHandler,
)
from sqrl.ipc.server import IPCServer, start_server

__all__ = [
    "IPCServer",
    "start_server",
    "IngestChunkHandler",
    "EmbedTextHandler",
    "SearchMemoriesHandler",
    "EvaluateMemoriesHandler",
]
