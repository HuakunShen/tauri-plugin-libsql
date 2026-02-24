import { drizzle } from "drizzle-orm/sqlite-proxy";
import { createDrizzleProxy, getConfig } from "tauri-plugin-libsql-api";
import * as schema from "./schema";

export type { Todo, NewTodo, TodoUpdate } from "./schema";

/**
 * Get the database path based on encryption config
 */
async function getDbPath(): Promise<string> {
  const config = await getConfig();
  return config.encrypted ? "sqlite:todos.enc.db" : "sqlite:todos.db";
}

/**
 * Create a Drizzle ORM instance with the libsql proxy
 */
export async function createDb() {
  const dbPath = await getDbPath();
  
  return drizzle<typeof schema>(
    createDrizzleProxy(dbPath),
    { schema, logger: true }
  );
}

/**
 * Singleton database instance
 */
let dbInstance: ReturnType<typeof drizzle<typeof schema>> | null = null;

/**
 * Get the singleton database instance
 */
export async function getDb() {
  if (!dbInstance) {
    dbInstance = await createDb();
  }
  return dbInstance;
}

/**
 * Reset the database instance (useful for testing)
 */
export function resetDb() {
  dbInstance = null;
}
