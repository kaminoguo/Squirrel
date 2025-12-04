# Cipher Analysis

Competitor analysis of [campfirein/cipher](https://github.com/campfirein/cipher) - the closest competitor to Squirrel with 3.3k GitHub stars.

## Overview

Cipher is an MCP-based memory layer for coding agents. Built by Byterover (Vietnam-based, bootstrapped).

**Key Stats:**
- 3.3k GitHub stars
- ~7.6k estimated downloads
- No disclosed VC funding
- Elastic License 2.0

## Architecture

```
AI Host (Claude/Cursor) ──calls MCP──> Cipher Server
                                           │
                                       MemAgent
                                           │
                    ┌──────────────────────┼──────────────────────┐
                    ▼                      ▼                      ▼
             EmbeddingManager      VectorStoreManager        LLMService
             (OpenAI/Ollama)       (Qdrant/Milvus/etc)      (decisions)
```

### Core Components

| Component | Purpose |
|-----------|---------|
| MemAgent | Orchestrates all memory operations |
| EmbeddingManager | Multi-provider embedding with circuit breaker |
| VectorStoreManager | 7+ backend support (Qdrant, Milvus, Chroma, FAISS, etc.) |
| DualCollectionManager | Separate knowledge + reflection collections |
| UnifiedToolManager | Exposes tools via MCP |

### Dual Memory System

1. **Knowledge Memory (System 1)**: Facts, code patterns, concepts
2. **Reflection Memory (System 2)**: Reasoning traces, decision steps (disabled by default)

## How It Works

### Memory Input

1. AI host calls `ask_cipher` MCP tool
2. Cipher's `extract_and_operate_memory` processes input
3. `isSignificantKnowledge()` filters trivial content (230 lines of regex)
4. LLM decides operation: ADD / UPDATE / DELETE / NONE
5. Stores in vector DB with embeddings

**Critical Flaw:** AI must DECIDE to call the tool. No passive learning.

### Memory Output

1. AI host calls `cipher_memory_search` MCP tool
2. Query embedded and similarity search performed
3. Results returned with similarity scores
4. No explanation of WHY results are relevant

**Critical Flaw:** AI must DECIDE to search. May forget.

### LLM Decision Prompts

The decision prompt is well-structured:

```
DECISION_PROMPT:
1) Compare with similar memories (semantic similarity, not keywords)
2) If none are similar (below threshold) -> ADD
3) If more correct/complete than a similar memory -> UPDATE
4) If redundant/already present -> NONE
5) If contradicts an existing memory -> DELETE
6) Always include confidence score (0.0–1.0)

Output: { "operation": "ADD|UPDATE|DELETE|NONE", "confidence": 0.0-1.0, "targetMemoryId": "..." }
```

## Strengths (Learn From)

### 1. LLM Decision Structure
The ADD/UPDATE/DELETE/NONE with confidence scoring is solid. Squirrel should adopt similar logic.

### 2. Circuit Breaker Pattern
When embedding fails, disables embeddings for session instead of crashing:
```typescript
if (embedError) {
    context.services.embeddingManager.handleRuntimeFailure(embedError, providerType);
    return { success: true, mode: 'chat-only', skipped: true };
}
```

### 3. Significance Filtering Concept
Filtering trivial content before storage is correct. Implementation (regex) is weak, but concept is right.

### 4. Workspace Memory
Team-shared memory with userId/projectId filtering. Good for future team features.

## Weaknesses (Avoid)

### 1. MCP-Only Input (Fatal Flaw)
```typescript
description: 'Use this tool to store new information...'
```
Relies on AI judgment to call tool. No passive learning.

### 2. No Task-Aware Explanations
Returns memories without explaining relevance:
```typescript
// Returns:
{ text, similarity, tags, timestamp }
// Missing: "This is relevant because..."
```

### 3. Bloated Files
- `extract_and_operate_memory.ts`: 1059 lines
- `mcp_handler.ts`: 723 lines
- `search_memory.ts`: 622 lines

### 4. Heavy Dependencies
262KB package-lock.json. Supports everything, optimizes nothing.

### 5. No Token Budget
No `context_budget_tokens` parameter. Can overflow context windows.

### 6. Regex-Based Filtering
230 lines of regex patterns for significance detection. Fragile, doesn't generalize.

## Squirrel vs Cipher

| Aspect | Cipher | Squirrel |
|--------|--------|----------|
| Input Method | AI calls MCP tool (unreliable) | Passive log watching (guaranteed) |
| Output Method | AI calls MCP tool (unreliable) | MCP tools (same limitation) |
| Explanations | None | "This is relevant because..." |
| Token Budget | None | `context_budget_tokens` |
| Architecture | Node.js monolith | Rust daemon + Python service |
| Distribution | npm | brew/cargo |
| Local-first | Optional | Core design |

## Key Takeaways for Squirrel

### Adopt
1. LLM decision prompt structure (ADD/UPDATE/DELETE/NONE + confidence)
2. Circuit breaker pattern for embedding failures
3. Significance filtering concept (but use LLM, not regex)

### Avoid
1. MCP-only input architecture
2. Multi-backend complexity (SQLite is enough)
3. 1000+ line files
4. npm/Node.js distribution
5. Regex-based content filtering

### Squirrel's Unique Advantages
1. **Passive learning** - watches logs, no AI action needed for input
2. **Task-aware context** - explains WHY each memory is relevant
3. **Budget-bounded** - fits any context window
4. **Cross-CLI** - automatically shares memory across tools
5. **Local-first** - all data on user's machine

## File Structure (Not Committed)

The Cipher repo is cloned locally for reference but excluded from git:
```
reference/learning/
├── CIPHER_ANALYSIS.md  (this file)
├── .gitignore          (excludes cipher/)
└── cipher/             (cloned, not committed)
    ├── src/
    │   ├── app/        (CLI, API, MCP, Web UI)
    │   └── core/       (brain, memory, vector, session)
    ├── memAgent/       (config files)
    └── docs/           (documentation)
```

## References

- Repository: https://github.com/campfirein/cipher
- Documentation: https://docs.byterover.dev/cipher/overview
- Company: https://byterover.dev
