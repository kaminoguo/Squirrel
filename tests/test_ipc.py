"""Tests for IPC server and handlers."""

import asyncio
import json
import tempfile
from pathlib import Path
from unittest.mock import patch

import pytest

from sqrl.ipc.handlers import (
    ComposeContextHandler,
    EmbedTextHandler,
    IngestChunkHandler,
    IPCError,
    SearchMemoriesHandler,
)
from sqrl.ipc.server import (
    METHOD_NOT_FOUND,
    PARSE_ERROR,
    IPCServer,
    RPCRequest,
    make_error,
    make_response,
)


class TestRPCRequest:
    """Test JSON-RPC request parsing."""

    def test_parse_valid_request(self):
        """Parse a valid JSON-RPC request."""
        data = json.dumps({
            "jsonrpc": "2.0",
            "method": "test_method",
            "params": {"key": "value"},
            "id": 1,
        })
        req = RPCRequest.from_json(data)

        assert req.method == "test_method"
        assert req.params == {"key": "value"}
        assert req.id == 1

    def test_parse_without_params(self):
        """Parse request with no params."""
        data = json.dumps({
            "jsonrpc": "2.0",
            "method": "test_method",
            "id": 2,
        })
        req = RPCRequest.from_json(data)

        assert req.method == "test_method"
        assert req.params == {}
        assert req.id == 2

    def test_parse_invalid_json(self):
        """Invalid JSON raises ValueError."""
        with pytest.raises(ValueError, match="Parse error"):
            RPCRequest.from_json("not valid json")

    def test_parse_wrong_version(self):
        """Wrong JSON-RPC version raises ValueError."""
        data = json.dumps({
            "jsonrpc": "1.0",
            "method": "test",
            "id": 1,
        })
        with pytest.raises(ValueError, match="Invalid JSON-RPC version"):
            RPCRequest.from_json(data)

    def test_parse_missing_method(self):
        """Missing method raises ValueError."""
        data = json.dumps({
            "jsonrpc": "2.0",
            "id": 1,
        })
        with pytest.raises(ValueError, match="Missing or invalid method"):
            RPCRequest.from_json(data)


class TestRPCResponse:
    """Test JSON-RPC response formatting."""

    def test_make_response(self):
        """Create success response."""
        response = make_response({"key": "value"}, 1)
        obj = json.loads(response)

        assert obj["jsonrpc"] == "2.0"
        assert obj["result"] == {"key": "value"}
        assert obj["id"] == 1

    def test_make_error(self):
        """Create error response."""
        from sqrl.ipc.server import RPCError

        error = RPCError(PARSE_ERROR, "Parse error")
        response = make_error(error, 1)
        obj = json.loads(response)

        assert obj["jsonrpc"] == "2.0"
        assert obj["error"]["code"] == PARSE_ERROR
        assert obj["error"]["message"] == "Parse error"
        assert obj["id"] == 1


class TestIPCServer:
    """Test IPC server functionality."""

    @pytest.mark.asyncio
    async def test_register_handler(self):
        """Register a method handler."""
        with tempfile.TemporaryDirectory() as tmpdir:
            socket_path = str(Path(tmpdir) / "test.sock")
            server = IPCServer(socket_path)

            def handler(params):
                return {"result": "ok"}

            server.register("test_method", handler)
            assert "test_method" in server.handlers

    @pytest.mark.asyncio
    async def test_handle_valid_request(self):
        """Handle a valid request."""
        with tempfile.TemporaryDirectory() as tmpdir:
            socket_path = str(Path(tmpdir) / "test.sock")
            server = IPCServer(socket_path)

            def handler(params):
                return {"echo": params.get("input")}

            server.register("echo", handler)

            request = json.dumps({
                "jsonrpc": "2.0",
                "method": "echo",
                "params": {"input": "hello"},
                "id": 1,
            })

            response = await server._handle_request(request)
            obj = json.loads(response)

            assert obj["result"]["echo"] == "hello"
            assert obj["id"] == 1

    @pytest.mark.asyncio
    async def test_handle_method_not_found(self):
        """Handle unknown method."""
        with tempfile.TemporaryDirectory() as tmpdir:
            socket_path = str(Path(tmpdir) / "test.sock")
            server = IPCServer(socket_path)

            request = json.dumps({
                "jsonrpc": "2.0",
                "method": "unknown",
                "params": {},
                "id": 1,
            })

            response = await server._handle_request(request)
            obj = json.loads(response)

            assert "error" in obj
            assert obj["error"]["code"] == METHOD_NOT_FOUND

    @pytest.mark.asyncio
    async def test_handle_async_handler(self):
        """Handle async handler."""
        with tempfile.TemporaryDirectory() as tmpdir:
            socket_path = str(Path(tmpdir) / "test.sock")
            server = IPCServer(socket_path)

            async def async_handler(params):
                await asyncio.sleep(0.01)
                return {"async": True}

            server.register("async_test", async_handler)

            request = json.dumps({
                "jsonrpc": "2.0",
                "method": "async_test",
                "params": {},
                "id": 1,
            })

            response = await server._handle_request(request)
            obj = json.loads(response)

            assert obj["result"]["async"] is True


class TestEmbedTextHandler:
    """Test IPC-002 embed_text handler."""

    @pytest.mark.asyncio
    async def test_embed_success(self):
        """Successful embedding generation."""
        mock_embedding = [0.1] * 1536

        with patch("sqrl.ipc.handlers.embed_text") as mock_embed:
            mock_embed.return_value = mock_embedding

            handler = EmbedTextHandler()
            result = await handler({"text": "test text"})

            assert result["embedding"] == mock_embedding
            mock_embed.assert_called_once()

    @pytest.mark.asyncio
    async def test_embed_empty_text(self):
        """Empty text raises error."""
        from sqrl.embeddings import ERROR_EMPTY_TEXT, EmbeddingError

        with patch("sqrl.ipc.handlers.embed_text") as mock_embed:
            mock_embed.side_effect = EmbeddingError(ERROR_EMPTY_TEXT, "Empty text")

            handler = EmbedTextHandler()

            with pytest.raises(IPCError) as exc_info:
                await handler({"text": ""})

            assert exc_info.value.code == ERROR_EMPTY_TEXT


class TestComposeContextHandler:
    """Test IPC-003 compose_context handler."""

    @pytest.mark.asyncio
    async def test_compose_basic(self):
        """Basic context composition."""
        handler = ComposeContextHandler()

        result = await handler({
            "task": "add webhook endpoint",
            "memories": [
                {"id": "mem-1", "kind": "invariant", "text": "Use httpx for HTTP"},
                {"id": "mem-2", "kind": "pattern", "text": "SSL errors → use httpx"},
            ],
            "token_budget": 400,
        })

        assert "context_prompt" in result
        assert "used_memory_ids" in result
        assert "mem-1" in result["used_memory_ids"]
        assert "mem-2" in result["used_memory_ids"]
        assert "httpx" in result["context_prompt"]

    @pytest.mark.asyncio
    async def test_compose_empty_task(self):
        """Empty task raises error."""
        handler = ComposeContextHandler()

        with pytest.raises(IPCError) as exc_info:
            await handler({"task": "", "memories": []})

        assert exc_info.value.code == -32010


class TestSearchMemoriesHandler:
    """Test IPC-004 search_memories handler."""

    @pytest.mark.asyncio
    async def test_search_empty_query(self):
        """Empty query raises error."""
        handler = SearchMemoriesHandler()

        with pytest.raises(IPCError) as exc_info:
            await handler({
                "project_id": "test",
                "query": "",
            })

        assert exc_info.value.code == -32012

    @pytest.mark.asyncio
    async def test_search_no_project(self):
        """Missing project raises error."""
        handler = SearchMemoriesHandler()

        with pytest.raises(IPCError) as exc_info:
            await handler({
                "query": "test query",
            })

        assert exc_info.value.code == -32010

    @pytest.mark.asyncio
    async def test_search_with_custom_fn(self):
        """Search with custom search function."""
        async def mock_search(**kwargs):
            return [{"id": "mem-1", "text": "result", "score": 0.9}]

        handler = SearchMemoriesHandler(search_fn=mock_search)

        result = await handler({
            "project_id": "test",
            "query": "test query",
            "top_k": 5,
        })

        assert len(result["results"]) == 1
        assert result["results"][0]["id"] == "mem-1"


class TestIngestChunkHandler:
    """Test IPC-001 ingest_chunk handler."""

    @pytest.mark.asyncio
    async def test_ingest_empty_chunk(self, monkeypatch):
        """Empty chunk raises error."""
        # Set required env vars (format: provider/model)
        monkeypatch.setenv("SQRL_STRONG_MODEL", "openrouter/test-model")
        monkeypatch.setenv("OPENROUTER_API_KEY", "test-key")

        handler = IngestChunkHandler()

        with pytest.raises(IPCError) as exc_info:
            await handler({
                "project_id": "test",
                "events": [],
            })

        assert exc_info.value.code == -32001

    @pytest.mark.asyncio
    async def test_ingest_missing_project(self, monkeypatch):
        """Missing project raises error."""
        monkeypatch.setenv("SQRL_STRONG_MODEL", "openrouter/test-model")
        monkeypatch.setenv("OPENROUTER_API_KEY", "test-key")

        handler = IngestChunkHandler()

        with pytest.raises(IPCError) as exc_info:
            await handler({
                "events": [{"ts": "2024-01-01", "role": "user", "kind": "message"}],
            })

        assert exc_info.value.code == -32002
