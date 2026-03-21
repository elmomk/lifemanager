# Life Manager PWA - Technical Specification

## 1. Overview

Mobile-first, offline-capable PWA consolidating five modules — To-Dos, Groceries, Shopee Pick-ups, Watchlist, and Cycle Tracker — into a single native-like interface.

**Stack:** Rust + Dioxus (Wasm), Tailwind CSS.

## 2. Tech Stack

| Layer | Choice |
|---|---|
| Framework | Dioxus (Rust → Wasm) |
| Styling | Tailwind CSS (dioxus-cli) |
| Icons | Lucide (SVG) |
| Storage | IndexedDB via `rexie` (offline-first) |
| Sync | Google Calendar API (OAuth 2.0) |
| Hosting | Static (Vercel / GitHub Pages / Cloudflare Pages) + `manifest.json` + Service Worker |

> **Storage note:** IndexedDB over LocalStorage — it handles binary blobs (Shopee images), has no 5 MB cap, and supports indexed queries for sorting/filtering.

## 3. UI/UX & Interaction Design

- **Design language:** Glassmorphism (`backdrop-filter: blur`), `rounded-2xl`/`rounded-3xl`, soft shadows.
- **Theming:** System-aware dark/light toggle.
- **Navigation:** Bottom tab bar (5 icons) + horizontal swipe to switch tabs with directional slide animations.

### Swipe Engine

| Direction | Action | Snap threshold |
|---|---|---|
| Right | Mark done / picked up | 120% translateX |
| Left | Delete | -120% translateX |

**Conflict resolution:** Vertical scroll cancels horizontal swipe; item-level swipe suppresses tab swipe.

## 4. Data Models & Modules

### Shared Traits

Most modules share `id`, `done`/`picked_up`, and optional `synced` state. A common trait reduces boilerplate:

```rust
trait Trackable {
    fn id(&self) -> u64;
    fn is_complete(&self) -> bool;
    fn is_synced(&self) -> bool;
}
```

**Sorting rule (all modules except Cycle):** Active items first, completed items below (reduced opacity, strikethrough).

---

### 4.1 To-Dos (`/todos`)

Quick-add chips for frequent tasks, optional due date, Google Calendar sync.

```rust
struct Todo {
    id: u64,
    text: String,
    date: Option<NaiveDate>,
    done: bool,
    synced: bool,
}
```

### 4.2 Groceries (`/groceries`)

Shopping list with quick-add chips (Milk, Eggs, ...), optional needed-by date, Google Calendar sync.

```rust
struct Grocery {
    id: u64,
    text: String,
    date: Option<NaiveDate>,
    done: bool,
    synced: bool,
}
```

> **Optimization:** `Todo` and `Grocery` are structurally identical. Consider a single generic `ChecklistItem` with a `category: ItemCategory` discriminant to eliminate duplication while keeping modules visually distinct.

```rust
enum ItemCategory { Todo, Grocery }

struct ChecklistItem {
    id: u64,
    text: String,
    date: Option<NaiveDate>,
    done: bool,
    synced: bool,
    category: ItemCategory,
}
```

### 4.3 Shopee Pick-ups (`/shopee`)

Track packages at convenience stores (7-11, FamilyMart). Log description, store, unlock code, optional image attachment. Google Calendar sync.

```rust
struct ShopeePackage {
    id: u64,
    title: String,
    store: Option<String>,
    code: Option<String>,
    image_blob_key: Option<String>, // IndexedDB blob reference
    picked_up: bool,
    synced: bool,
}
```

> **Optimization:** Store images as IndexedDB blobs referenced by key instead of inlining base64 data URLs. This avoids doubling memory usage from base64 encoding and keeps serialization fast.

### 4.4 Watchlist (`/watchlist`)

Track media with type classification and quick-add chips.

```rust
#[derive(Clone, PartialEq)]
enum MediaType { Movie, Series, Anime, Cartoon }

struct WatchItem {
    id: u64,
    text: String,
    media_type: MediaType,
    done: bool,
    synced: bool,
}
```

### 4.5 Cycle Tracker (`/period`)

Log menstrual cycles, attach symptoms, predict next start date.

```rust
struct Cycle {
    id: u64,
    start_date: NaiveDate,
    end_date: Option<NaiveDate>,
    symptoms: Vec<String>,
}
```

**Prediction engine:** Next expected start = most recent `start_date` + average cycle length (default 28 days, refined as data accumulates). Display as countdown.

**Swipe overrides:** Right-swipe (mark done) disabled. Left-swipe deletes history entries only.

## 5. Google Calendar Sync

1. **Auth:** OAuth 2.0 via GCP Web Client ID. Token stored in IndexedDB.
2. **Trigger:** Manual "Sync" button in header, or auto-sync ~2 s after item creation (debounced).
3. **Mapping:**

   | Module | Calendar representation |
   |---|---|
   | Todos / Groceries | All-day event on a dedicated "Life Manager" calendar |
   | Shopee | Timed reminder or all-day event |
   | Watchlist | Optional (no date-sensitive urgency) |

4. **State update:** On `200 OK`, set `synced = true` → show cloud-check icon.
5. **Offline resilience:** Queue failed syncs in IndexedDB; retry on next connectivity event (`navigator.onLine` + `online` event listener).

## 6. Phase 2 Enhancements

- **NLP input parsing:** Parse strings like "Buy eggs next Tuesday" → auto-populate text + date fields.
- **Client-side OCR (Wasm):** Extract unlock code and store name from Shopee SMS screenshots.
- **Geolocation notifications:** Browser Geolocation API triggers local push notification near saved pickup locations.
