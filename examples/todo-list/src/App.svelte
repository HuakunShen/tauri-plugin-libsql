<script lang="ts">
  import { onMount } from "svelte";
  import { resolve } from "@tauri-apps/api/path";
  import { Database, getConfig, migrate } from "tauri-plugin-libsql-api";
  import { getDb, type Todo, type TodoUpdate } from "$lib/db";
  import * as schema from "$lib/schema";
  import { desc, eq } from "drizzle-orm";
  
  // Shadcn-style components
  import Button from "$lib/components/Button.svelte";
  import Input from "$lib/components/Input.svelte";
  import Card from "$lib/components/Card.svelte";
  import CardHeader from "$lib/components/CardHeader.svelte";
  import CardTitle from "$lib/components/CardTitle.svelte";
  import CardDescription from "$lib/components/CardDescription.svelte";
  import CardContent from "$lib/components/CardContent.svelte";
  import Checkbox from "$lib/components/Checkbox.svelte";
  import Badge from "$lib/components/Badge.svelte";
  import Alert from "$lib/components/Alert.svelte";

  // Bundle migration SQL files at build time
  const migrations = import.meta.glob<string>("../drizzle/*.sql", {
    eager: true,
    query: "?raw",
    import: "default",
  });

  let todos: Todo[] = [];
  let newTodo = "";
  let loading = true;
  let error = "";
  let encrypted = false;
  let dbPath = "";        // filename only, e.g. "todos.db"
  let dbAbsPath = "";     // resolved for display, e.g. "/Users/.../todos.db"

  onMount(async () => {
    try {
      const config = await getConfig();
      encrypted = config.encrypted;
      
      // Determine database path based on encryption
      const dbFile = config.encrypted ? "todos.enc.db" : "todos.db";
      dbPath = dbFile;
      dbAbsPath = await resolve(dbFile);
      const dbFullPath = `sqlite:${dbFile}`;
      
      // Load the database first
      await Database.load(dbFullPath);
      
      // Run migrations AFTER database is loaded
      await migrate(dbFullPath, migrations);
      
      await loadTodos();
      loading = false;
    } catch (e) {
      error = `Failed to initialize database: ${e}`;
      loading = false;
      console.error(e);
    }
  });

  async function loadTodos() {
    try {
      const db = await getDb();
      todos = await db.query.todos.findMany({
        orderBy: desc(schema.todos.createdAt),
      });
    } catch (e) {
      error = `Failed to load todos: ${e}`;
      console.error(e);
    }
  }

  async function addTodo() {
    if (!newTodo.trim()) return;

    try {
      const db = await getDb();
      await db.insert(schema.todos).values({
        title: newTodo.trim(),
      });
      newTodo = "";
      await loadTodos();
    } catch (e) {
      error = `Failed to add todo: ${e}`;
      console.error(e);
    }
  }

  async function toggleTodo(todo: Todo) {
    try {
      const db = await getDb();
      await db
        .update(schema.todos)
        .set({ completed: todo.completed ? 0 : 1 } as TodoUpdate)
        .where(eq(schema.todos.id, todo.id));
      await loadTodos();
    } catch (e) {
      error = `Failed to update todo: ${e}`;
      console.error(e);
    }
  }

  async function deleteTodo(id: number) {
    try {
      const db = await getDb();
      await db.delete(schema.todos).where(eq(schema.todos.id, id));
      await loadTodos();
    } catch (e) {
      error = `Failed to delete todo: ${e}`;
      console.error(e);
    }
  }

  async function deleteAllTodos() {
    try {
      const db = await getDb();
      await db.delete(schema.todos);
      await loadTodos();
    } catch (e) {
      error = `Failed to delete all todos: ${e}`;
      console.error(e);
    }
  }

  function handleSubmit(e: Event) {
    e.preventDefault();
    addTodo();
  }

  $: completedCount = todos.filter((t) => t.completed).length;
  $: totalCount = todos.length;
</script>

<main class="min-h-screen bg-background p-4 sm:p-8">
  <div class="mx-auto max-w-xl">
    <Card class="border-border/50 shadow-xl">
      <CardHeader class="space-y-2">
        <div class="flex items-center justify-between">
          <CardTitle class="flex items-center gap-3">
            <span class="text-3xl">📝</span>
            <span>Todo List</span>
          </CardTitle>
          {#if encrypted}
            <Badge class="border-success/30 bg-success/10 text-success">
              <svg class="mr-1 h-3 w-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
              </svg>
              Encrypted
            </Badge>
          {/if}
        </div>
        <CardDescription>
          Built with Tauri + libsql + Drizzle ORM
        </CardDescription>
      </CardHeader>

      <CardContent class="space-y-6">
        {#if loading}
          <div class="flex items-center justify-center py-12">
            <div class="flex items-center gap-3 text-muted-foreground">
              <svg class="h-5 w-5 animate-spin" fill="none" viewBox="0 0 24 24">
                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
              </svg>
              <span>Loading database...</span>
            </div>
          </div>
        {:else if error}
          <Alert variant="destructive">
            <div class="flex items-start gap-3">
              <svg class="mt-0.5 h-4 w-4 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
              <span>{error}</span>
            </div>
          </Alert>
        {:else}
          <form on:submit={handleSubmit} class="flex gap-3">
            <Input
              type="text"
              bind:value={newTodo}
              placeholder="What needs to be done?"
              class="flex-1"
            />
            <Button
              type="submit"
              disabled={!newTodo.trim()}
              class="shrink-0"
            >
              <svg class="mr-2 h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
              </svg>
              Add
            </Button>
          </form>

          {#if todos.length > 0}
            <div class="flex items-center justify-between border-b border-border/50 pb-3 text-sm text-muted-foreground">
              <Badge variant="secondary" class="bg-secondary/50">
                {totalCount} total
              </Badge>
              <Badge variant="secondary" class="bg-success/10 text-success border-success/20">
                {completedCount} completed
              </Badge>
            </div>

            <ul class="space-y-2">
              {#each todos as todo (todo.id)}
                <li
                  class="group flex items-center gap-3 rounded-lg border border-border/50 bg-secondary/20 p-4 transition-all hover:bg-secondary/30 hover:border-border {todo.completed ? 'opacity-60' : ''}"
                >
                  <Checkbox
                    checked={Boolean(todo.completed)}
                    onchange={() => toggleTodo(todo)}
                    class="shrink-0"
                  />
                  <span class="flex-1 text-sm {todo.completed ? 'line-through text-muted-foreground' : 'text-foreground'}">
                    {todo.title}
                  </span>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    onclick={() => deleteTodo(todo.id)}
                    class="h-8 w-8 opacity-0 group-hover:opacity-100 transition-opacity text-muted-foreground hover:text-destructive hover:bg-destructive/10"
                  >
                    <svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                    </svg>
                  </Button>
                </li>
              {/each}
            </ul>

            <Button
              type="button"
              variant="outline"
              onclick={deleteAllTodos}
              class="w-full border-destructive/30 text-destructive hover:bg-destructive/10 hover:text-destructive"
            >
              <svg class="mr-2 h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
              </svg>
              Delete All Todos
            </Button>
          {:else}
            <div class="flex flex-col items-center justify-center py-12 text-center">
              <div class="mb-4 rounded-full bg-muted p-4">
                <svg class="h-8 w-8 text-muted-foreground" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" />
                </svg>
              </div>
              <p class="text-muted-foreground">No todos yet. Add one above!</p>
            </div>
          {/if}
        {/if}
      </CardContent>
    </Card>

    <div class="mt-6 text-center">
      <p class="text-xs text-muted-foreground">
        {#if encrypted}
          Data stored with AES-256 encryption
        {:else}
          Data stored unencrypted
        {/if}
      </p>
      {#if dbAbsPath}
        <p class="mt-1 break-all font-mono text-xs text-muted-foreground/60">{dbAbsPath}</p>
      {/if}
    </div>
  </div>
</main>
