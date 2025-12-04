# MCP Memory Service Analysis

Competitor analysis of [doobidoo/mcp-memory-service](https://github.com/doobidoo/mcp-memory-service) - a mature MCP-based memory system with sophisticated hooks integration.

## Overview

MCP Memory Service is an MCP server providing semantic memory and persistent storage for Claude Desktop/Code with multiple storage backends.

**Key Stats:**
- Version: 8.44.0
- Language: Python (FastAPI + MCP)
- Storage: SQLite-Vec / Cloudflare / Hybrid
- Embedding: all-MiniLM-L6-v2 (384-dim, ONNX)

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  MCP Layer (8 tools)                                         │
│  store_memory, retrieve_memory, recall_memory, etc.          │
├─────────────────────────────────────────────────────────────┤
│  HTTP Layer (FastAPI + OAuth 2.1)                            │
│  REST API + Web UI Dashboard + SSE streaming                 │
├─────────────────────────────────────────────────────────────┤
│  MemoryService (Business Logic)                              │
├─────────────────────────────────────────────────────────────┤
│  Storage Abstraction (Factory Pattern)                       │
│  ├─ SQLite-Vec (local, 5ms reads)                            │
│  ├─ Cloudflare (D1 + Vectorize, cloud)                       │
│  └─ Hybrid (local + cloud sync)                              │
├─────────────────────────────────────────────────────────────┤
│  Consolidation System ("Dream-Inspired")                     │
│  Decay → Associations → Compression → Forgetting             │
└─────────────────────────────────────────────────────────────┘
```

## Hooks System (Claude Code Integration)

### Multi-Phase Retrieval Pipeline

```
session-start.js Hook
├─ Phase 0: Git Context (recent commits)
├─ Phase 1: Recent Memories (last week)
├─ Phase 2: Tagged Memories (key-decisions, architecture)
└─ Phase 3: Fallback General Context
```

### Scoring Formula (memory-scorer.js)

```
Score = Σ (weight × factor) + bonuses

Factors:
  • timeDecay (0.25) - exponential decay by days
  • tagRelevance (0.35) - project/framework match
  • contentRelevance (0.15) - keyword matching
  • contentQuality (0.25) - penalize generic content

Bonuses:
  • typeBonus - decision/architecture get extra points
  • recencyBonus - +0.15 for last 7 days
```

### Adaptive Weight Calibration

```javascript
// If memories are stale (median > 30 days):
weights.timeDecay = 0.50;      // Up from 0.25
weights.tagRelevance = 0.20;   // Down from 0.35
```

## What's Shit

### 1. MCP-Only Input (Fatal Flaw)
Same problem as Cipher. AI must decide to call `store_memory`. No passive learning. All the sophisticated scoring means nothing if memories never get stored.

### 2. Over-Engineered Scoring (726 lines)
```javascript
calculateTimeDecay()
calculateTagRelevance()
calculateContentRelevance()
calculateContentQuality()
calculateTypeBonus()
calculateRecencyBonus()
analyzeMemoryAgeDistribution()
calculateAdaptiveGitWeight()
calculateConversationRelevance()
```
A simple `score = recency * 0.5 + similarity * 0.5` would get 80% of the value.

### 3. Formatting Bloat (1219 lines)
`context-formatter.js` has ANSI colors, tree rendering, markdown conversion, multiple output modes. The AI doesn't care about pretty boxes.

### 4. No Task-Awareness
Returns: `"Decided to use SQLite-vec for better performance"`
Should return: `"Relevant because you're debugging a performance issue"`

### 5. 24 Memory Types (Taxonomy Paralysis)
```
note, reference, document, guide, session, implementation,
analysis, troubleshooting, test, fix, feature, release...
```
Users won't remember which to use. Our 4 types are clearer.

### 6. Windows Hook Bug Unfixed
SessionStart hooks hang Claude Code on Windows. Documented workaround instead of fix.

## What's Good (Learn From)

### 1. Project Affinity Hard Filter
```javascript
if (!hasProjectTag && tagScore < 0.3) {
    finalScore = 0;  // Exclude entirely, not just down-weight
}
```
Prevents cross-project memory pollution.

### 2. Content Quality Penalty
```javascript
const genericPatterns = [/Topics Discussed.*implementation.*\.\.\.$/s];
if (isGeneric) return 0.05;  // Heavy penalty
```
Filters out noise from generic session summaries.

### 3. Multi-Phase Retrieval
Git context → Recent → Tagged → Fallback. Smart degradation prevents empty context.

### 4. Adaptive Calibration
System adapts weights based on memory DB staleness. If everything is old, boost recency weight.

### 5. Code Execution Path
```python
from mcp_memory_service.api import consolidate
result = consolidate('weekly')  # 15 tokens vs 150 MCP tool
```
90% token reduction by bypassing MCP for heavy operations.

### 6. ONNX Embeddings
25MB model, ~10ms inference, no PyTorch dependency. Right tradeoff for CLI tools.

## Squirrel vs MCP Memory Service

| Aspect | MCP Memory Service | Squirrel |
|--------|-------------------|----------|
| **Input** | MCP tool (AI decides) | Passive log watching |
| **Output** | Hooks + MCP | Router + Files/Hooks |
| **Scoring** | 726 lines, over-engineered | Simple weighted formula |
| **Task-Aware** | No | Yes ("relevant because...") |
| **Memory Types** | 24 types | 4 types (style, fact, pitfall, recipe) |
| **Token Budget** | No | Yes |
| **Storage** | SQLite-Vec/Cloudflare/Hybrid | SQLite only |
| **Embedding** | ONNX (good) | TBD (steal this) |

## Key Takeaways for Squirrel

### Adopt
1. **Project affinity hard filter** - Exclude cross-project memories entirely
2. **Content quality penalty** - Filter generic summaries
3. **Multi-phase retrieval** - Git → Recent → Tagged → Fallback
4. **Adaptive calibration** - Adjust weights based on memory staleness
5. **ONNX embeddings** - Fast, no PyTorch dependency
6. **Code execution path** - Bypass MCP for heavy operations

### Avoid
1. MCP-only input architecture
2. 700+ line scoring files
3. 1200+ line formatting files
4. 24 memory types
5. Web UI dashboard (unnecessary for CLI)

### Squirrel's Unique Advantages
1. **Passive learning** - Watches logs, no AI action needed
2. **Task-aware context** - Explains WHY each memory is relevant
3. **Budget-bounded** - Fits any context window
4. **Simple architecture** - 4 memory types, deterministic routing
5. **Router-first** - Our model decides, not host LLM

## File Structure (Not Committed)

```
reference/learning/
├─ MCP_MEMORY_SERVICE_ANALYSIS.md  (this file)
├─ CIPHER_ANALYSIS.md
├─ .gitignore                       (excludes cloned repos)
└─ mcp-memory-service/              (cloned, not committed)
    ├─ src/mcp_memory_service/      (Python backend)
    ├─ claude-hooks/                (JS hooks for Claude Code)
    │   ├─ core/
    │   │   ├─ session-start.js     (main hook)
    │   │   └─ memory-retrieval.js
    │   └─ utilities/
    │       ├─ memory-scorer.js     (726 lines of scoring)
    │       └─ context-formatter.js (1219 lines of formatting)
    └─ docs/
```

## References

- Repository: https://github.com/doobidoo/mcp-memory-service
- Version analyzed: 8.44.0
