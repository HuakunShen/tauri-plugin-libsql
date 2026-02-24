---
name: tauri-plugin-libsql
description: Use tauri-plugin-libsql for SQLite database access in Tauri apps with Drizzle ORM, browser-safe migrations, and optional AES-256-CBC encryption. Use when working on this plugin's source, writing apps that consume it, adding schema changes, debugging migration or query errors, or configuring encryption.
version: 1.0.0
license: MIT
metadata:
  tags:
    - tauri
    - sqlite
    - libsql
    - drizzle-orm
    - encryption
    - migrations
---

# tauri-plugin-libsql

SQLite plugin for Tauri apps via libsql. Provides encryption, Drizzle ORM integration, and a browser-safe migration runner.

## Key Files

```
guest-js/index.ts     — Database class, getConfig, re-exports
guest-js/drizzle.ts   — createDrizzleProxy, createDrizzleProxyWithEncryption
guest-js/migrate.ts   — migrate() function
src/commands.rs       — Rust command handlers: load, execute, select, close
src/wrapper.rs        — DbConnection (resolves base_path, calls libsql Builder)
src/desktop.rs        — Config struct, base_path resolution
src/lib.rs            — Plugin init, command registration
examples/tauri-app/   — Reference demo: Drizzle + migrations + encryption
```

## Critical: Why a Custom Migrator Exists

`drizzle-orm/sqlite-proxy/migrator` calls `readMigrationFiles()` which reads from the filesystem at runtime. That API does not exist in a Tauri WebView (browser context). The plugin's `migrate()` function instead receives SQL content that Vite bundles into the app at build time via `import.meta.glob`.

## Startup Sequence (always in this order)

```typescript
// 1. Open/create the database file
await Database.load('sqlite:myapp.db');

// 2. Run pending migrations — must come before any table queries
await migrate('sqlite:myapp.db', migrations);

// 3. Now safe to use Drizzle
const db = drizzle(createDrizzleProxy('sqlite:myapp.db'), { schema });
```

Querying before `migrate()` causes "no such table" errors.

## Full Usage Pattern

### schema.ts

```typescript
import { integer, sqliteTable, text } from 'drizzle-orm/sqlite-core';
import { sql } from 'drizzle-orm';

export const todos = sqliteTable('todos', {
  id: integer('id').primaryKey({ autoIncrement: true }),
  title: text('title').notNull(),
  completed: integer('completed').notNull().default(0),
  createdAt: text('created_at').default(sql`(current_timestamp)`),
});

export type Todo = typeof todos.$inferSelect;
```

### drizzle.config.ts

```typescript
import { defineConfig } from 'drizzle-kit';

export default defineConfig({
  dialect: 'sqlite',
  schema: './src/lib/schema.ts',
  out: './drizzle',
});
```

### Generate migrations

```bash
npx drizzle-kit generate
# or
bun run db:generate
```

This creates `drizzle/0000_xxx.sql`, `drizzle/0001_xxx.sql`, etc. Commit these files.

### App startup (Svelte example)

```typescript
import { Database, migrate, createDrizzleProxy } from 'tauri-plugin-libsql-api';
import { drizzle } from 'drizzle-orm/sqlite-proxy';
import * as schema from './schema';

// import.meta.glob path is relative to this source file
const migrations = import.meta.glob<string>('../drizzle/*.sql', {
  eager: true,
  query: '?raw',
  import: 'default',
});

const dbPath = 'sqlite:myapp.db';
await Database.load(dbPath);
await migrate(dbPath, migrations);

const db = drizzle(createDrizzleProxy(dbPath), { schema });
```

## Database Location

Relative paths resolve against `base_path` in the Rust plugin config:
- **Default**: `std::env::current_dir()` — where the Tauri process is launched from
- **Custom**: set `base_path: Some(PathBuf::from(...))` in `Config`
- Absolute paths are used as-is
- `:memory:` → in-memory database

Relative paths containing `..` are normalised and validated. A path that would escape `base_path` (e.g. `sqlite:../../etc/passwd`) is rejected with `InvalidDbUrl`.

The demo app (`src-tauri/src/lib.rs`) explicitly sets `base_path: Some(cwd)` so the DB lands next to where `bun run tauri dev` is invoked.

## Encryption

### Option 1: Plugin-level (recommended — key stays in Rust)

```rust
// src-tauri/src/lib.rs
let config = tauri_plugin_libsql::Config {
    base_path: Some(cwd),
    encryption: Some(tauri_plugin_libsql::EncryptionConfig {
        cipher: tauri_plugin_libsql::Cipher::Aes256Cbc,
        key: my_32_byte_vec, // Vec<u8>
    }),
};
tauri::Builder::default()
    .plugin(tauri_plugin_libsql::init_with_config(config))
    ...
```

The demo reads the key from `LIBSQL_ENCRYPTION_KEY` env var and pads/truncates to 32 bytes.

### Option 2: Per-database (key passed from frontend)

```typescript
const db = await Database.load({
  path: 'sqlite:secrets.db',
  encryption: {
    cipher: 'aes256cbc',
    key: Array.from(myUint8Array32), // must be exactly 32 bytes
  },
});
```

### With Drizzle + encryption

```typescript
const db = drizzle(
  createDrizzleProxyWithEncryption({
    path: 'sqlite:encrypted.db',
    encryption: { cipher: 'aes256cbc', key: myKey },
  }),
  { schema }
);
```

## Adding a New Column / Table (Migration Workflow)

1. Edit `src/lib/schema.ts`
2. Run `npx drizzle-kit generate` — creates a new numbered `.sql` file in `drizzle/`
3. Commit the new migration file
4. On next app launch, `migrate()` detects and applies it automatically

Never manually edit existing migration files. Add new ones only.

## Common Errors

| Error | Cause | Fix |
|-------|-------|-----|
| `no such table: todos` | `migrate()` not called before queries, or migration files missing | Check startup sequence; run `drizzle-kit generate` |
| `DatabaseNotLoaded` | Query sent before `Database.load()` | Call `Database.load()` first |
| `DatabaseNotLoaded` after close | `createDrizzleProxy` `loaded` flag doesn't reset on external close | Recreate the proxy after calling `Database.close()` |
| `Migration X failed` | Bad SQL in a migration file | Check the `.sql` file; fix schema definition |
| `path '...' escapes the base directory` | Relative path contains `..` that exits `base_path` | Use a path that stays within the configured base directory |
| DB file not found | Wrong working directory | Check `base_path` config or launch directory |
| Encryption error on open | Wrong key for existing encrypted DB | Use exact same key as when DB was created |

## Plugin Architecture

```
Frontend (TS)                    Rust Plugin
─────────────────                ─────────────────────────
Database.load()      ──invoke──▶ commands::load()
  migrate()          ──invoke──▶ commands::execute() (DDL)
  db.execute()       ──invoke──▶ commands::execute()
  db.select()        ──invoke──▶ commands::select()
                                   │
                                 wrapper::DbConnection
                                   │ base_path.join(relative_path)
                                   │ LibsqlBuilder::new_local()
                                   │   .encryption_config()
                                   │   .build()
                                   ▼
                                 libsql (SQLite file)
```

## Building the JS Package

After changing `guest-js/` files:

```bash
npm run build   # at repo root — runs rollup, outputs dist-js/
```

The demo app references the plugin as `file:../../` so it picks up the built output automatically.

## Permissions

Every Tauri command needs a permission. Default set in `permissions/default.toml`. To allow all commands in a capability:

```json
{
  "permissions": [
    "libsql:allow-load",
    "libsql:allow-execute",
    "libsql:allow-select",
    "libsql:allow-close"
  ]
}
```
