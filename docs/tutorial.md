# Life Manager — Developer Tutorial

This tutorial walks through the codebase and teaches you how to work with and extend Life Manager.

---

## Prerequisites

- **Rust** (stable toolchain) with `wasm32-unknown-unknown` target
- **Dioxus CLI**: `cargo install dioxus-cli`
- **Node.js** (for Tailwind CSS)
- **SQLite** (bundled via `rusqlite`, no separate install needed)

## Getting Started

### 1. Install dependencies

```bash
# Install Rust Wasm target
rustup target add wasm32-unknown-unknown

# Install Dioxus CLI
cargo install dioxus-cli

# Install Tailwind CSS
npm install
```

### 2. Run the dev server

Open two terminals:

```bash
# Terminal 1: Dioxus dev server (hot reload)
dx serve

# Terminal 2: Tailwind CSS watcher
npm run tailwind
```

The app is available at `http://localhost:8080/lifemanager/`.

### 3. Production build

```bash
dx build --release
```

---

## Tutorial: Adding a New Module

Let's walk through adding a hypothetical **"Notes"** module step by step. This covers every layer of the stack.

### Step 1: Define the Model

Create `src/models/note.rs`:

```rust
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub text: String,
    pub created_at: f64,
}
```

Register it in `src/models/mod.rs`:

```rust
pub mod note;
pub use note::Note;
```

### Step 2: Create the Database Table

In `src/server/db.rs`, add a `CREATE TABLE` statement inside `init()`:

```rust
conn.execute_batch(
    "CREATE TABLE IF NOT EXISTS notes (
        id TEXT PRIMARY KEY,
        user_id TEXT NOT NULL,
        text TEXT NOT NULL,
        created_at REAL NOT NULL
    );"
)?;
```

### Step 3: Write Server Functions

Create `src/api/notes.rs`:

```rust
use dioxus::prelude::*;
use crate::models::Note;

#[server(ListNotes)]
pub async fn list_notes() -> Result<Vec<Note>, ServerFnError> {
    use axum::extract::FromRequestParts;
    use crate::server::{auth::user_from_headers, db::pool};

    let headers: axum::http::HeaderMap = extract().await?;
    let user_id = user_from_headers(&headers);
    let conn = pool().get()?;

    let mut stmt = conn.prepare(
        "SELECT id, text, created_at FROM notes
         WHERE user_id = ?1
         ORDER BY created_at DESC"
    )?;

    let items = stmt.query_map([&user_id], |row| {
        Ok(Note {
            id: row.get(0)?,
            text: row.get(1)?,
            created_at: row.get(2)?,
        })
    })?.filter_map(|r| r.ok()).collect();

    Ok(items)
}

#[server(AddNote)]
pub async fn add_note(text: String) -> Result<(), ServerFnError> {
    use axum::extract::FromRequestParts;
    use crate::server::{auth::user_from_headers, db::pool};

    let headers: axum::http::HeaderMap = extract().await?;
    let user_id = user_from_headers(&headers);
    let conn = pool().get()?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis() as f64;

    conn.execute(
        "INSERT INTO notes (id, user_id, text, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![id, user_id, text, now],
    )?;
    Ok(())
}

#[server(DeleteNote)]
pub async fn delete_note(id: String) -> Result<(), ServerFnError> {
    use axum::extract::FromRequestParts;
    use crate::server::{auth::user_from_headers, db::pool};

    let headers: axum::http::HeaderMap = extract().await?;
    let user_id = user_from_headers(&headers);
    let conn = pool().get()?;
    conn.execute(
        "DELETE FROM notes WHERE id = ?1 AND user_id = ?2",
        rusqlite::params![id, user_id],
    )?;
    Ok(())
}
```

Register in `src/api/mod.rs`:

```rust
pub mod notes;
```

> **Key pattern**: Every server function extracts headers, gets the user_id, then gets a connection from the pool. Server-only imports (`rusqlite`, `uuid`, etc.) go *inside* the function body so they're excluded from the Wasm build.

### Step 4: Build the Page Component

Create `src/pages/notes.rs`:

```rust
use dioxus::prelude::*;
use crate::api::notes::*;
use crate::components::swipe_item::SwipeItem;

#[component]
pub fn Notes() -> Element {
    let mut items = use_signal(Vec::new);
    let mut input = use_signal(String::new);
    let mut refresh = use_signal(|| 0u32);

    // Load items on mount and when refresh changes
    use_effect(move || {
        let _ = refresh();
        spawn(async move {
            if let Ok(result) = list_notes().await {
                items.set(result);
            }
        });
    });

    let on_add = move |evt: FormEvent| {
        evt.prevent_default();
        let text = input().trim().to_string();
        if text.is_empty() { return; }
        spawn(async move {
            if add_note(text).await.is_ok() {
                input.set(String::new());
                refresh += 1;
            }
        });
    };

    rsx! {
        div { class: "px-4 space-y-4",
            // Add form
            form { onsubmit: on_add, class: "flex gap-2",
                input {
                    r#type: "text",
                    placeholder: "New note...",
                    value: "{input}",
                    oninput: move |e| input.set(e.value()),
                    class: "flex-1 px-4 py-2 rounded-xl bg-white/70 dark:bg-gray-800/70 backdrop-blur",
                }
                button {
                    r#type: "submit",
                    class: "px-4 py-2 bg-blue-500 text-white rounded-xl",
                    "Add"
                }
            }

            // Item list
            for item in items() {
                SwipeItem {
                    key: "{item.id}",
                    done: false,
                    on_swipe_left: {
                        let id = item.id.clone();
                        move |_| {
                            let id = id.clone();
                            spawn(async move {
                                if delete_note(id).await.is_ok() {
                                    refresh += 1;
                                }
                            });
                        }
                    },
                    "{item.text}"
                }
            }
        }
    }
}
```

Register in `src/pages/mod.rs`:

```rust
pub mod notes;
pub use notes::Notes;
```

### Step 5: Add the Route

In `src/route.rs`, add the new route variant:

```rust
#[route("/notes")]
Notes {},
```

### Step 6: Add a Tab Bar Entry

In `src/components/tab_bar.rs`, add the navigation link. You'll also want to add an icon in `src/components/icons.rs`.

### Step 7: Test It

```bash
dx serve
```

Navigate to `/lifemanager/notes` — you should see your new module.

---

## Common Patterns Reference

### The Refresh Signal Pattern

Every page uses a `refresh` signal to trigger data reloads:

```rust
let mut refresh = use_signal(|| 0u32);

// In use_effect — reading refresh() creates a dependency
use_effect(move || {
    let _ = refresh();
    spawn(async move { /* fetch data */ });
});

// After any mutation
refresh += 1;  // triggers the effect to re-run
```

### Server Function Imports

Server-only crates (`rusqlite`, `uuid`, `tokio`, etc.) must be imported **inside** the `#[server]` function body, not at the top of the file. This prevents them from being compiled into the Wasm bundle:

```rust
#[server(MyFunction)]
pub async fn my_function() -> Result<(), ServerFnError> {
    // ✅ Import here — only compiled for server
    use crate::server::db::pool;

    let conn = pool().get()?;
    // ...
    Ok(())
}
```

### SwipeItem Usage

`SwipeItem` wraps list items and provides swipe gestures:

```rust
SwipeItem {
    key: "{item.id}",
    done: item.done,                        // controls opacity/strikethrough
    on_swipe_right: move |_| { /* toggle */ },  // optional — omit to disable
    on_swipe_left: move |_| { /* delete */ },   // required
    // children go here as the body
    "Item text"
}
```

- Right swipe (≥120px) → green background, check icon → typically "mark done"
- Left swipe (≤-120px) → red background, trash icon → typically "delete"

### QuickAddChips Usage

```rust
QuickAddChips {
    items: vec!["Option A".into(), "Option B".into(), "Option C".into()],
    on_select: move |text: String| {
        input.set(text);
    },
}
```

### cfg Gates

```rust
// Server-only module
#[cfg(not(target_arch = "wasm32"))]
pub mod server;

// Client-only code
#[cfg(target_arch = "wasm32")]
{
    // IndexedDB, DOM APIs, etc.
}
```

---

## Deployment

### Local Development

```bash
dx serve          # http://localhost:8080/lifemanager/
npm run tailwind  # watches input.css → assets/main.css
```

### Production

1. Build: `dx build --release`
2. Start the server (the release binary serves both frontend and API)
3. Configure Nginx (see `nginx.conf`) to reverse-proxy port 7000 → 8080
4. Configure Tailscale serve to expose port 7000 at `/lifemanager`

```
Internet → Tailscale (:443) → Nginx (:7000) → dx serve (:8080)
```

The Tailscale layer provides HTTPS and injects the `Tailscale-User-Login` header for authentication — no separate auth system needed.
