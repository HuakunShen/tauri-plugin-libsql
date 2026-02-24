# PROJECT KNOWLEDGE BASE: tauri-plugin-libsql

**Generated:** 2025-02-25 06:00 CST  
**Commit:** ef66553  
**Branch:** main

## OVERVIEW

Tauri plugin for libsql (Turso's SQLite fork) with AES-256-CBC encryption, Drizzle ORM support, and browser-safe migrations. Enables SQLite in Tauri apps without Node.js runtime dependency.

## STRUCTURE

```
.
├── src/              # Rust plugin (Tauri commands, libsql wrapper)
├── guest-js/         # TypeScript API (Database class, Drizzle proxy, migrations)
├── permissions/      # Tauri permission definitions
├── examples/         # Demo apps (todo-list)
├── Cargo.toml        # Rust dependencies & features
├── package.json      # JS package config
└── SKILL.md          # Comprehensive AI skill reference
```

## WHERE TO LOOK

| Task | Location | Notes |
|------|----------|-------|
| Add Tauri command | `src/commands.rs` | Follow existing patterns, add to `lib.rs` invoke_handler |
| Modify database operations | `src/wrapper.rs` | `DbConnection` handles local/replica/remote modes |
| Change encryption | `src/models.rs`, `src/wrapper.rs` | `EncryptionConfig` → libsql cipher |
| Add JS API method | `guest-js/index.ts` | `Database` class, use `invoke("plugin:libsql\|cmd")` |
| Drizzle integration | `guest-js/drizzle.ts` | Proxy pattern for sqlite-proxy driver |
| Migration runner | `guest-js/migrate.ts` | Parses `import.meta.glob` bundled SQL |
| Error types | `src/error.rs` | `thiserror` enum, serializable for Tauri |
| Config | `src/desktop.rs` | `base_path`, default encryption |
| Permissions | `permissions/` | Add to `default.toml` for new commands |

## CODE MAP

| Symbol | Type | Location | Role |
|--------|------|----------|------|
| `init` / `init_with_config` | fn | `src/lib.rs:25` | Plugin entry |
| `load` / `execute` / `select` | cmd | `src/commands.rs` | Tauri command handlers |
| `DbConnection` | struct | `src/wrapper.rs:16` | libsql wrapper with catch_unwind |
| `DbInstances` | struct | `src/wrapper.rs:290` | Connection pool (Arc<Mutex<HashMap>>) |
| `Database` | class | `guest-js/index.ts:54` | Frontend API |
| `createDrizzleProxy` | fn | `guest-js/drizzle.ts:80` | sqlite-proxy callback |
| `migrate` | fn | `guest-js/migrate.ts:79` | Browser-safe migration runner |
| `Error` | enum | `src/error.rs:5` | Plugin error types |

## CONVENTIONS

**Rust:**
- Use `thiserror` for errors (must impl `Serialize` for Tauri IPC)
- Wrap libsql builder in `catch_unwind` (malformed URLs panic internally)
- Release mutex locks before awaiting (`let conn = { ... lock().await ... }.clone()`)
- Feature-gate: `#[cfg(feature = "replication")]` / `#[cfg(feature = "encryption")]`
- Path resolution: `sqlite:` prefix stripped, `..` normalized, checked against `base_path`

**TypeScript:**
- Use `invoke<T>("plugin:libsql|command", args)` pattern
- Drizzle proxy transforms `IndexMap` rows to array-per-row format
- Migrations use Vite `import.meta.glob` at build time (no runtime fs access)

## ANTI-PATTERNS

- **DON'T** use `drizzle-orm/sqlite-proxy/migrator` — it reads filesystem at runtime (WebView has no fs)
- **DON'T** query before `migrate()` — causes "no such table" errors
- **DON'T** use `execute_batch` for embedded replicas — use explicit `BEGIN`/`COMMIT` transaction
- **DON'T** pass encryption keys from frontend if avoidable — use plugin-level config instead
- **DON'T** let libsql builder panic unhandled — always wrap in `catch_unwind`

## FEATURE FLAGS

| Feature | Default | Description |
|---------|---------|-------------|
| `core` | ✅ | Local SQLite databases |
| `encryption` | ✅ | AES-256-CBC encryption |
| `replication` | ❌ | Turso embedded replica sync |
| `remote` | ❌ | Pure remote connections |

## COMMANDS

```bash
# Build JS package
npm run build        # or: rollup -c

# Generate Drizzle migrations
npx drizzle-kit generate

# Dev (from example)
cd examples/todo-list && bun run tauri dev
```

## STARTUP SEQUENCE

```typescript
// 1. Load database
await Database.load('sqlite:app.db');

// 2. Run migrations (before any queries!)
await migrate('sqlite:app.db', migrations);

// 3. Now safe to use Drizzle
const db = drizzle(createDrizzleProxy('sqlite:app.db'), { schema });
```

## NOTES

- **SKILL.md exists** — comprehensive reference for AI assistants; copy to `.claude/skills/` for Claude Code
- **Encryption**: 32-byte key required (AES-256-CBC), can be plugin-level or per-database
- **Turso**: Embedded replica needs `replication` feature; initial sync on load, manual `db.sync()` for updates
- **Migrations**: SQL bundled at build time via Vite — no runtime filesystem dependency
- **Error handling**: libsql internal panics caught via `catch_unwind` and converted to `Error::InvalidDbUrl`
