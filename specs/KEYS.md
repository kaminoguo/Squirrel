# Squirrel Declarative Keys

Registry of declarative keys for facts. Same key + different value triggers deterministic invalidation (no LLM needed).

## Key Format

```
<scope>.<category>.<property>
```

- **scope**: `project` or `user`
- **category**: domain grouping (db, api, language, etc.)
- **property**: specific attribute

## Project-Scoped Keys (KEY-P-*)

Stored with `project_id` set. Authoritative for project environment.

| ID | Key | Description | Example Values |
|----|-----|-------------|----------------|
| KEY-P-001 | `project.db.engine` | Database engine | PostgreSQL, MySQL, SQLite, MongoDB |
| KEY-P-002 | `project.db.version` | Database version | 15, 8.0, 3.x |
| KEY-P-003 | `project.db.orm` | ORM/query builder | Prisma, SQLAlchemy, TypeORM, Diesel |
| KEY-P-004 | `project.api.framework` | API framework | FastAPI, Express, Rails, Actix |
| KEY-P-005 | `project.ui.framework` | UI framework | React, Vue, Svelte, None |
| KEY-P-006 | `project.language.main` | Primary language | Python, TypeScript, Rust, Go |
| KEY-P-007 | `project.language.version` | Language version | 3.12, 5.0, 1.75 |
| KEY-P-008 | `project.test.framework` | Test framework | pytest, jest, cargo test |
| KEY-P-009 | `project.test.command` | Test run command | `pytest`, `npm test`, `cargo test` |
| KEY-P-010 | `project.build.command` | Build command | `npm run build`, `cargo build` |
| KEY-P-011 | `project.auth.method` | Auth mechanism | JWT, session, OAuth, API key |
| KEY-P-012 | `project.package_manager` | Package manager | npm, pnpm, yarn, pip, uv, cargo |
| KEY-P-013 | `project.deploy.platform` | Deploy target | Vercel, AWS, Railway, self-hosted |
| KEY-P-014 | `project.ci.platform` | CI system | GitHub Actions, GitLab CI, CircleCI |

## User-Scoped Keys (KEY-U-*)

Stored with `project_id = NULL` in global db. Stable user preferences.

| ID | Key | Description | Example Values |
|----|-----|-------------|----------------|
| KEY-U-001 | `user.preferred_style` | Coding style | async_await, callbacks, sync |
| KEY-U-002 | `user.preferred_language` | Favorite language | Python, TypeScript, Rust |
| KEY-U-003 | `user.strict_null_checks` | Null handling | true, false |
| KEY-U-004 | `user.comment_style` | Comment preference | minimal, detailed, jsdoc |
| KEY-U-005 | `user.error_handling` | Error pattern | exceptions, result_types, errors |
| KEY-U-006 | `user.test_style` | Testing approach | tdd, after_impl, minimal |
| KEY-U-007 | `user.naming_convention` | Naming style | snake_case, camelCase |

## Promotion Rules

User preferences (`user.*` keys) require evidence before becoming keyed facts:

| Rule | Threshold |
|------|-----------|
| Minimum success signals | 3 |
| Recency window | 7 days |
| Confidence threshold | 0.8 |

Until promotion threshold is met, observations are stored as unkeyed facts with `evidence_source` tracking.

## Conflict Resolution

### Keyed Facts (Deterministic)

```
IF new_fact.key == existing_fact.key
   AND new_fact.value != existing_fact.value:

   existing_fact.status = 'invalidated'
   existing_fact.valid_to = now()
   existing_fact.superseded_by = new_fact.id
```

No LLM judgment needed. Pure key-value match.

### Unkeyed Facts (LLM-Assisted)

For facts without declarative keys, LLM judges semantic conflict:

1. Vector search for similar existing facts (top 5)
2. If similarity > 0.85, LLM evaluates conflict
3. High confidence conflict → invalidate old
4. Low confidence → keep both, recency weighted in retrieval

## Fast Path Retrieval

Keyed facts support direct lookup without embeddings:

```sql
SELECT * FROM memories
WHERE key = 'project.db.engine'
  AND project_id = ?
  AND status = 'active'
```

Use this for environment queries before falling back to vector search.

## Access Logging

All keyed fact lookups logged with `access_type = 'key_lookup'`:

```json
{
    "key": "project.db.engine",
    "value": "PostgreSQL",
    "hit": true
}
```
