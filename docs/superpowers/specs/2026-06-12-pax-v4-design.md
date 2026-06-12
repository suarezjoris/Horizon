# Pax v4 — Proactivité Logicielle Passive

**Date :** 2026-06-12  
**Statut :** Approuvé  
**Scope :** `pax_daemon.rs`, `main.rs`, `forge_daemon.rs`, `index.html`, `src/modules/`

---

## Contexte

Le daemon Pax existe et fonctionne pour le cas *chat-idle* : après 5s de silence dans
le chat, il scanne le vault, évalue les lacunes via LLM, et injecte une question dans
l'historique du chat. Le timing post-question est fixé à 10min (mécanique).

La v4 ajoute trois choses :
1. **Timing adaptatif** — le cooldown s'ajuste selon la qualité de la dernière réponse.
2. **Déclencheurs software** — startup app, fin d'un cycle Forge, changement fichier workspace.
3. **Bannière non-intrusive** — les déclencheurs non-chat émettent une card flottante, pas une injection dans le chat.

---

## Architecture : Dual-path

```
Chemin A — Chat-idle (existant, enrichi)
  run_pax() loop → poll 2s → idle ≥ IDLE_THRESHOLD → scan vault → pax-question → chat

Chemin B — Events software (nouveau)
  PaxEvent channel ──→ run_pax_banner() task → scan vault → pax-banner → bannière UI
       ↑
  [Startup 45s]  [ForgeStep: distill/refine]  [WorkspaceChange: fichier .md/.rs/.py]
```

Les deux chemins partagent uniquement `AVOIDED_NOTES` (liste des notes récemment traitées).
Les cooldowns sont indépendants : `LAST_QUALITY` pour le chemin A, `BANNER_LAST_SENT` pour le chemin B.

---

## Section 1 — Nouveaux types et état partagé

```rust
// pax_daemon.rs

pub enum PaxEvent {
    Startup,
    ForgeStep { message: String },
    WorkspaceChange,
}

// Sender stocké en app state dans main.rs
pub struct PaxEventSender(pub tokio::sync::mpsc::Sender<PaxEvent>);

#[derive(Clone, Copy)]
enum ReplyQuality { Positive, Neutral, Ignored }

static LAST_QUALITY:  Lazy<Mutex<ReplyQuality>> = Lazy::new(|| Mutex::new(ReplyQuality::Neutral));
static PAX_ASKED_AT:  Lazy<AtomicU64>           = Lazy::new(|| AtomicU64::new(0));
static BANNER_LAST_SENT: Lazy<AtomicU64>        = Lazy::new(|| AtomicU64::new(0));
```

---

## Section 2 — Timing adaptatif (Chemin A)

### Détection qualité dans `touch_activity()`

```rust
if was_waiting && !is_rejection(msg) {
    let q = if msg.len() > 60 || msg.contains('?') {
        ReplyQuality::Positive
    } else {
        ReplyQuality::Neutral
    };
    *LAST_QUALITY.lock().unwrap() = q;
}
```

### Timeout "Ignored" dans `run_pax()` loop

Si Pax attend une réponse depuis >15min sans que `touch_activity` ait été appelé,
on cesse d'attendre et on enregistre `Ignored`.

```rust
if waiting {
    let elapsed = now.saturating_sub(PAX_ASKED_AT.load(Ordering::Relaxed));
    if elapsed > 900 {
        WAITING_FOR_REPLY.store(false, Ordering::Relaxed);
        *LAST_QUALITY.lock().unwrap() = ReplyQuality::Ignored;
    }
    continue;
}
```

### Calcul du cooldown adaptatif

```rust
fn adaptive_cooldown(now_secs: u64) -> u64 {
    let jitter = now_secs % 60; // pseudo-aléatoire sans dépendance externe
    match *LAST_QUALITY.lock().unwrap() {
        ReplyQuality::Positive => 180 + jitter / 2,  // ~3-4 min
        ReplyQuality::Neutral  => 540 + jitter,      // ~9-10 min
        ReplyQuality::Ignored  => 1200 + jitter * 3, // ~20-23 min
    }
}
```

Remplace le check `since_q < QUESTION_COOLDOWN_SECS` par `since_q < adaptive_cooldown(now)`.

On enregistre `PAX_ASKED_AT` au moment où Pax pose sa question (après `WAITING_FOR_REPLY.store(true)`).

---

## Section 3 — Déclencheurs software (Chemin B)

### 3a. Tâche bannière

```rust
pub async fn run_pax_banner(mut rx: Receiver<PaxEvent>, app: AppHandle) {
    const BANNER_COOLDOWN: u64 = 1800; // 30min minimum entre bannières

    while let Some(event) = rx.recv().await {
        let now = now_secs();
        let last = BANNER_LAST_SENT.load(Ordering::Relaxed);
        if now.saturating_sub(last) < BANNER_COOLDOWN { continue; }

        let s = settings::load();
        let index = embeddings::load_index(&s.embeddings_path);
        let gaps = find_gaps(&s.vault_path, &index);
        if gaps.is_empty() { continue; }

        // Tente d'acquérir le GPU — abandon si occupé
        use tauri::Manager;
        let _permit = {
            let q = app.state::<crate::vram_queue::VramQueue>();
            match q.try_acquire("pax-banner") {
                Some(p) => p,
                None => continue,
            }
        };

        if let Some(q) = evaluate_and_generate(&gaps[0], &s.agents.light_model).await {
            BANNER_LAST_SENT.store(now, Ordering::Relaxed);
            let _ = app.emit("pax-banner", serde_json::json!({ "question": q }));
        }
    }
}
```

### 3b. Trigger Startup (main.rs)

Dans `main.rs` setup, après le spawn de Pax :

```rust
if s.agents.pax_enabled {
    let tx = pax_tx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(45)).await;
        let _ = tx.send(PaxEvent::Startup).await;
    });
}
```

45s de délai pour laisser l'utilisateur ouvrir l'app et s'installer.

### 3c. Trigger ForgeStep (forge_daemon.rs)

Après les deux opérations significatives de Forge, si des faits ont été traités :

```rust
// Après distill_vanguard_to_hubs() et refine_messy_notes()
if total_facts > 0 {
    use tauri::Manager;
    if let Some(sender) = app.try_state::<crate::pax_daemon::PaxEventSender>() {
        let _ = sender.0.try_send(PaxEvent::ForgeStep {
            message: format!("Forge processed {} facts", total_facts),
        });
    }
}
```

`try_send` (non-bloquant) — Forge ne se bloque jamais sur Pax.

### 3d. Trigger WorkspaceChange (run_pax_banner)

Watcher inotify sur `settings.agent_workspace`, lancé depuis `run_pax_banner` au démarrage.
`run_pax_banner` reçoit aussi le `Sender` original en paramètre pour pouvoir le cloner dans le thread watcher :

```rust
pub async fn run_pax_banner(
    mut rx: Receiver<PaxEvent>,
    tx: tokio::sync::mpsc::Sender<PaxEvent>,  // pour le watcher
    app: AppHandle,
)

let workspace = PathBuf::from(&settings::load().agent_workspace);
let tx_ws = tx.clone();

std::thread::spawn(move || {
    use notify::{Watcher, RecursiveMode, recommended_watcher};
    let (ntx, nrx) = std::sync::mpsc::channel();
    if let Ok(mut w) = recommended_watcher(ntx) {
        let _ = w.watch(&workspace, RecursiveMode::NonRecursive);
        for res in nrx {
            if let Ok(event) = res {
                let is_code = event.paths.iter().any(|p| {
                    matches!(p.extension().and_then(|e| e.to_str()), Some("md"|"rs"|"py"|"txt"))
                });
                if is_code {
                    let _ = tx_ws.blocking_send(PaxEvent::WorkspaceChange);
                }
            }
        }
    }
});
```

Le BANNER_COOLDOWN de 30min empêche le spam si l'utilisateur édite rapidement.

---

## Section 4 — UI Bannière

### HTML (index.html, près de `#hub-proposal-banner`)

```html
<div id="pax-banner" style="display:none">
  <span class="pax-banner-label">◈ Pax</span>
  <span id="pax-banner-question"></span>
  <div class="pax-banner-actions">
    <button id="pax-banner-open">Ouvrir le chat</button>
    <button id="pax-banner-dismiss">✕</button>
  </div>
</div>
```

### JS (llm.js ou pax.js)

```js
listen('pax-banner', (event) => {
    const q = event.payload.question;
    document.getElementById('pax-banner-question').textContent = q;
    document.getElementById('pax-banner').style.display = 'flex';
    pendingPaxQuestion = q;
});

document.getElementById('pax-banner-open')?.addEventListener('click', () => {
    document.getElementById('pax-banner').style.display = 'none';
    // Switch vers l'onglet LLM et injecter la question dans le chat
    showTab('llm');
    addBubble('ai', pendingPaxQuestion);
    // Met WAITING_FOR_REPLY via la boucle naturelle — pas de commande Tauri nécessaire
    pendingPaxQuestion = null;
});

document.getElementById('pax-banner-dismiss')?.addEventListener('click', () => {
    document.getElementById('pax-banner').style.display = 'none';
    pendingPaxQuestion = null;
    // Pas d'action Rust — le timeout 15min dans run_pax marquera Ignored si pertinent
});
```

---

## Section 5 — Bug fix : pax dans toggle_agent_daemon

Le match dans `toggle_agent_daemon` (main.rs:557) n'a pas de cas `"pax"`.
À ajouter :

```rust
"pax" => {
    tokio::spawn(async move {
        pax_daemon::run_pax(app_clone, flag_clone).await;
    });
}
```

---

## Fichiers modifiés

| Fichier | Nature du changement |
|---|---|
| `src-tauri/src/pax_daemon.rs` | `ReplyQuality`, `PAX_ASKED_AT`, `BANNER_LAST_SENT`, `adaptive_cooldown()`, timeout Ignored, `PaxEvent`, `PaxEventSender`, `run_pax_banner()`, watcher workspace |
| `src-tauri/src/main.rs` | channel mpsc, `manage(PaxEventSender)`, spawn `run_pax_banner`, startup trigger, fix `toggle_agent_daemon` |
| `src-tauri/src/forge_daemon.rs` | `try_send(PaxEvent::ForgeStep)` après distill et refine |
| `src/index.html` | div `#pax-banner` |
| `src/modules/llm.js` | listener `pax-banner`, inject dans chat, dismiss |

---

## Contraintes

- `try_send` partout depuis Forge/workspace — Pax ne bloque jamais un autre agent.
- VRAM : `run_pax_banner` passe par `VramQueue::try_acquire` — abandon si GPU occupé.
- Pas de nouvelle dépendance : `notify` est déjà dans `Cargo.toml`.
- `AVOIDED_NOTES` partagé entre les deux chemins — une note rejetée n'est jamais proposée dans une bannière non plus.
