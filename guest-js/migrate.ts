import { invoke } from '@tauri-apps/api/core'

/**
 * A map of migration file paths to their SQL content.
 * Typically produced by Vite's `import.meta.glob` at build time.
 *
 * @example
 * ```ts
 * const migrations = import.meta.glob('./drizzle/*.sql', {
 *   eager: true,
 *   query: '?raw',
 *   import: 'default',
 * }) satisfies MigrationFiles
 * ```
 */
export type MigrationFiles = Record<string, string>

export interface MigrateOptions {
  /**
   * Name of the table used to track applied migrations.
   * @default '__drizzle_migrations'
   */
  migrationsTable?: string
}

interface ParsedMigration {
  filename: string
  sql: string
  index: number
  isFolderV3: boolean
}

function parseMigrationPath(path: string): Omit<ParsedMigration, 'sql'> | null {
  const segments = path.split(/[\\/]+/).filter(Boolean)
  const fileName = segments[segments.length - 1]

  if (!fileName) {
    return null
  }

  if (fileName === 'migration.sql' && segments.length >= 2) {
    const folderName = segments[segments.length - 2]
    const match = folderName.match(/^(\d+)[_\-].*$/)

    if (match) {
      return {
        filename: `${folderName}.sql`,
        index: parseInt(match[1], 10),
        isFolderV3: true,
      }
    }
  }

  const match = fileName.match(/^(\d+)[_\-].*\.sql$/)
  if (!match) {
    return null
  }

  return {
    filename: fileName,
    index: parseInt(match[1], 10),
    isFolderV3: false,
  }
}

function parseMigrations(files: MigrationFiles): ParsedMigration[] {
  const migrations: ParsedMigration[] = []

  for (const [path, sql] of Object.entries(files)) {
    const parsedPath = parseMigrationPath(path)
    if (parsedPath && sql) {
      migrations.push({
        filename: parsedPath.filename,
        sql: sql as string,
        index: parsedPath.index,
        isFolderV3: parsedPath.isFolderV3,
      })
    }
  }

  return migrations.sort((a, b) => a.index - b.index)
}

/**
 * Runs pending Drizzle ORM migrations against a libsql database.
 *
 * Because the Tauri plugin runs inside a browser context, standard
 * drizzle-kit migrate (which reads from the filesystem) cannot be used.
 * Instead, bundle your migration SQL files at build time with Vite's
 * `import.meta.glob` and pass them here.
 *
 * Call this AFTER `Database.load()` and BEFORE any queries.
 *
 * @param dbPath - The database path (e.g. "sqlite:app.db")
 * @param migrationFiles - SQL file contents keyed by path, from `import.meta.glob`
 * @param options - Optional configuration
 *
 * @example
 * ```ts
 * import { Database, migrate } from 'tauri-plugin-libsql-api'
 *
 * // Bundle migrations at build time (glob pattern relative to this file)
 * const migrations = import.meta.glob('../drizzle/*.sql', {
 *   eager: true,
 *   query: '?raw',
 *   import: 'default',
 * })
 *
 * await Database.load('sqlite:app.db')
 * await migrate('sqlite:app.db', migrations)
 * ```
 */
export async function migrate(
  dbPath: string,
  migrationFiles: MigrationFiles,
  options: MigrateOptions = {},
): Promise<void> {
  const table = options.migrationsTable ?? '__drizzle_migrations'

  // Ensure migrations tracking table exists
  await invoke('plugin:libsql|execute', {
    db: dbPath,
    query: `CREATE TABLE IF NOT EXISTS ${table} (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      hash TEXT NOT NULL UNIQUE,
      created_at DATETIME DEFAULT CURRENT_TIMESTAMP
    )`,
    values: [],
  })

  // Get already-applied migrations
  const applied = await invoke<Array<{ hash: string }>>('plugin:libsql|select', {
    db: dbPath,
    query: `SELECT hash FROM ${table}`,
    values: [],
  })
  const appliedSet = new Set(applied.map((r) => r.hash))

  // Parse and sort migration files
  const migrations = parseMigrations(migrationFiles)
  const legacyFolderV3Migration = appliedSet.has('migration.sql')
    ? migrations.find((migration) => migration.isFolderV3)
    : undefined

  for (const migration of migrations) {
    if (appliedSet.has(migration.filename)) {
      continue
    }

    if (migration.filename === legacyFolderV3Migration?.filename) {
      continue
    }

    // Split on semicolons to get individual statements.
    // Note: this is a naive split — semicolons inside string literals will
    // cause incorrect splits. drizzle-kit generated SQL does not produce this.
    const statements = migration.sql
      .split(';')
      .map((s) => s.trim())
      .filter((s) => s.length > 0)

    // Record the migration in the same transaction as the schema changes so
    // a partial failure leaves no trace. One invoke for the entire migration.
    const safeName = migration.filename.replace(/'/g, "''")
    statements.push(`INSERT INTO ${table} (hash) VALUES ('${safeName}')`)

    await invoke('plugin:libsql|batch', {
      db: dbPath,
      queries: statements,
    })
  }
}
