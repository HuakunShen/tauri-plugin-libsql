import { invoke } from "@tauri-apps/api/core";

/** Cipher types for encryption */
export type Cipher = "aes256cbc";

/** Encryption configuration */
export interface EncryptionConfig {
  /** Cipher to use for encryption */
  cipher: Cipher;
  /** Encryption key as bytes */
  key: number[] | Uint8Array;
}

/** Options for loading a database */
export interface LoadOptions {
  /** Database path (e.g., "sqlite:test.db" or just "test.db") */
  path: string;
  /** Optional encryption configuration */
  encryption?: EncryptionConfig;
}

/** Result of an execute operation */
export interface QueryResult {
  /** Number of rows affected */
  rowsAffected: number;
  /** Last inserted row ID */
  lastInsertId: number;
}

/**
 * **Database**
 *
 * The `Database` class serves as the primary interface for
 * communicating with the libsql plugin.
 */
export class Database {
  /** The database path */
  path: string;

  constructor(path: string) {
    this.path = path;
  }

  /**
   * **load**
   *
   * A static initializer which connects to the underlying database and
   * returns a `Database` instance once a connection to the database is established.
   *
   * # Path Format
   *
   * The path is relative to `tauri::path::BaseDirectory::AppConfig` and must start with `sqlite:`.
   *
   * @example
   * ```ts
   * // Simple load
   * const db = await Database.load("sqlite:test.db");
   *
   * // With encryption
   * const db = await Database.load({
   *   path: "sqlite:encrypted.db",
   *   encryption: {
   *     cipher: "aes-256-cbc",
   *     key: [1, 2, 3, ...] // 32 bytes for AES-256
   *   }
   * });
   * ```
   */
  static async load(pathOrOptions: string | LoadOptions): Promise<Database> {
    const options =
      typeof pathOrOptions === "string"
        ? { path: pathOrOptions }
        : pathOrOptions;

    const _path = await invoke<string>("plugin:libsql|load", { options });
    return new Database(_path);
  }

  /**
   * **get**
   *
   * A static initializer which synchronously returns an instance of
   * the Database class while deferring the actual database connection
   * until the first invocation or selection on the database.
   *
   * @example
   * ```ts
   * const db = Database.get("sqlite:test.db");
   * ```
   */
  static get(path: string): Database {
    return new Database(path);
  }

  /**
   * **execute**
   *
   * Passes a SQL expression to the database for execution.
   *
   * @example
   * ```ts
   * // INSERT example
   * const result = await db.execute(
   *   "INSERT INTO todos (id, title, status) VALUES ($1, $2, $3)",
   *   [todos.id, todos.title, todos.status]
   * );
   * // UPDATE example
   * const result = await db.execute(
   *   "UPDATE todos SET title = $1, completed = $2 WHERE id = $3",
   *   [todos.title, todos.status, todos.id]
   * );
   * ```
   */
  async execute(query: string, bindValues?: unknown[]): Promise<QueryResult> {
    const result = await invoke<QueryResult>("plugin:libsql|execute", {
      db: this.path,
      query,
      values: bindValues ?? [],
    });
    return result;
  }

  /**
   * **select**
   *
   * Passes in a SELECT query to the database for execution.
   *
   * @example
   * ```ts
   * const result = await db.select<{ id: number; title: string }[]>(
   *   "SELECT * FROM todos WHERE id = $1",
   *   [id]
   * );
   * ```
   */
  async select<T>(query: string, bindValues?: unknown[]): Promise<T> {
    const result = await invoke<T>("plugin:libsql|select", {
      db: this.path,
      query,
      values: bindValues ?? [],
    });
    return result;
  }

  /**
   * **close**
   *
   * Closes the database connection pool.
   *
   * @example
   * ```ts
   * const success = await db.close()
   * ```
   *
   * @param db - Optionally state the name of a database if you are managing more than one. Otherwise, all database pools will be in scope.
   */
  async close(db?: string): Promise<boolean> {
    const success = await invoke<boolean>("plugin:libsql|close", { db });
    return success;
  }
}

/** Plugin configuration info */
export interface ConfigInfo {
  /** Whether encryption is enabled */
  encrypted: boolean;
}

/**
 * Get plugin configuration info
 * 
 * @returns ConfigInfo with encryption status
 */
export async function getConfig(): Promise<ConfigInfo> {
  return invoke<ConfigInfo>("plugin:libsql|get_config");
}

// Re-export for drizzle integration
export { createDrizzleProxy } from "./drizzle";

// Re-export migration utility
export { migrate } from "./migrate";
export type { MigrationFiles, MigrateOptions } from "./migrate";
