import { expect, mock, test } from "bun:test";

interface HarnessState {
  hashes: string[];
  userColumns: string[];
}

interface InvokeArgs {
  query?: string;
  queries?: string[];
}

interface Harness {
  readonly hashes: string[];
  readonly userColumns: string[];
  invoke(command: string, args: InvokeArgs): Promise<unknown>;
}

let activeHarness: Harness | undefined;

mock.module("@tauri-apps/api/core", () => ({
  invoke(command: string, args: InvokeArgs) {
    if (!activeHarness) {
      throw new Error("No active migration harness");
    }

    return activeHarness.invoke(command, args);
  },
}));

type Migrate = (
  dbPath: string,
  migrationFiles: Record<string, string>,
) => Promise<void>;

let migratePromise: Promise<Migrate> | undefined;

function getMigrate() {
  migratePromise ??= import("../dist-js/index.js").then(
    (module) => module.migrate as Migrate,
  );
  return migratePromise;
}

function createHarness({
  hashes = [],
  userColumns = [],
}: Partial<HarnessState> = {}): Harness {
  const state: HarnessState = {
    hashes: [...hashes],
    userColumns: [...userColumns],
  };

  function applyQuery(next: HarnessState, query: string) {
    const trimmed = query.trim();

    if (/^CREATE TABLE users\b/i.test(trimmed)) {
      next.userColumns = ["id"];
      return;
    }

    if (/^ALTER TABLE users ADD COLUMN name\b/i.test(trimmed)) {
      if (!next.userColumns.includes("name")) {
        next.userColumns.push("name");
      }
      return;
    }

    const insertMatch = trimmed.match(
      /^INSERT INTO \S+ \(hash\) VALUES \('((?:''|[^'])*)'\)$/i,
    );

    if (insertMatch) {
      const hash = insertMatch[1].replace(/''/g, "'");
      if (next.hashes.includes(hash)) {
        throw new Error("UNIQUE constraint failed: __drizzle_migrations.hash");
      }
      next.hashes.push(hash);
    }
  }

  return {
    get hashes() {
      return state.hashes;
    },
    get userColumns() {
      return state.userColumns;
    },
    async invoke(command, args) {
      if (command === "plugin:libsql|execute") {
        return { rowsAffected: 0, lastInsertId: 0 };
      }

      if (command === "plugin:libsql|select") {
        return state.hashes.map((hash) => ({ hash }));
      }

      if (command === "plugin:libsql|batch") {
        const next: HarnessState = {
          hashes: [...state.hashes],
          userColumns: [...state.userColumns],
        };

        for (const query of args.queries ?? []) {
          applyQuery(next, query);
        }

        state.hashes = next.hashes;
        state.userColumns = next.userColumns;
        return undefined;
      }

      throw new Error(`Unexpected command: ${command}`);
    },
  };
}

test("applies Drizzle v3 folder migrations with unique parent-folder hashes", async () => {
  activeHarness = createHarness();
  const migrate = await getMigrate();

  await migrate("sqlite:app.db", {
    "/src/db/migrations/20260508135710_init/migration.sql":
      "CREATE TABLE users (id INTEGER PRIMARY KEY);",
    "/src/db/migrations/20260605110000_add_users/migration.sql":
      "ALTER TABLE users ADD COLUMN name TEXT;",
  });

  expect(activeHarness.hashes).toEqual([
    "20260508135710_init.sql",
    "20260605110000_add_users.sql",
  ]);
  expect(activeHarness.userColumns).toEqual(["id", "name"]);
});

test("keeps flat migration filenames as hashes", async () => {
  activeHarness = createHarness();
  const migrate = await getMigrate();

  await migrate("sqlite:app.db", {
    "./drizzle/0000_init.sql": "CREATE TABLE users (id INTEGER PRIMARY KEY);",
    "./drizzle/0001_add_name.sql": "ALTER TABLE users ADD COLUMN name TEXT;",
  });

  expect(activeHarness.hashes).toEqual(["0000_init.sql", "0001_add_name.sql"]);
  expect(activeHarness.userColumns).toEqual(["id", "name"]);
});

test("continues Drizzle v3 migrations after a legacy migration.sql hash", async () => {
  activeHarness = createHarness({
    hashes: ["migration.sql"],
    userColumns: ["id"],
  });
  const migrate = await getMigrate();

  await migrate("sqlite:app.db", {
    "\\src\\db\\migrations\\20260508135710_init\\migration.sql":
      "CREATE TABLE users (id INTEGER PRIMARY KEY);",
    "\\src\\db\\migrations\\20260605110000_add_users\\migration.sql":
      "ALTER TABLE users ADD COLUMN name TEXT;",
  });

  expect(activeHarness.hashes).toEqual([
    "migration.sql",
    "20260605110000_add_users.sql",
  ]);
  expect(activeHarness.userColumns).toEqual(["id", "name"]);
});
