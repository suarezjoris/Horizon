# agent/core.py
import re
import json
from typing import Callable
import ollama
from agent.memory import get_context
from agent.tools import web_search, fetch_url
from vault.manager import write_note, append_note, read_note
from config import MODEL, OLLAMA_HOST

_SYSTEM = """\
Tu es l'IA personnelle de Joris — locale, sans filtre, directe.
Tu le connais bien grâce à ta mémoire. Utilise-la activement.

COMPORTEMENT :
- Toujours "tu", jamais "vous"
- Appelle-le Joris quand c'est naturel
- Si ta mémoire contient des infos pertinentes, fais des liens explicites
- Si tu manques d'infos sur Joris lui-même (ses projets, ses ressentis, ses choix), pose UNE question personnelle sur lui — jamais sur des sujets généraux que tu peux chercher toi-même
- Ton ton : direct, honnête, sans corporate, un peu de profondeur

OUTILS :
  ACTION: web_search("ta requête")   → recherche DuckDuckGo
  ACTION: fetch_url("https://...")   → lire une URL
Pour utiliser un outil, réponds UNIQUEMENT avec la ligne ACTION.
Tu peux enchaîner plusieurs outils avant ta réponse finale.

<memory>
{memory}
</memory>
"""

_EXTRACT = """\
Tu es un système de mémoire neuronal autonome. Analyse cette conversation et construis le graphe de connaissance.

FICHIERS EXISTANTS dans la vault :
{existing_files}

RÈGLES :
1. memory/*.md → faits atomiques avec [[wikilinks]] vers les sujets connus
   Ex: "- Projet actif : [[Wing It]] — app mobile anonyme"
   Ex: "- Utilise [[Figma]] pour le design"
   MAX une ligne par fait. INTERDIT : transcriptions, dialogues, résumés longs.

2. knowledge/<Sujet>.md → page dédiée si un sujet mérite développement
   Crée TOUJOURS un knowledge/ quand un projet, outil, concept est mentionné en détail.
   Utilise des [[wikilinks]] dans le contenu pour relier aux concepts liés.
   Ex: knowledge/Wing It.md → mentionne [[Figma]], [[React Native]], [[Joris]]

3. Lie les nouveaux faits aux fichiers EXISTANTS quand pertinent.
4. conversations/ → NE PAS utiliser.
5. Si rien de nouveau → retourne [].

Retourne UNIQUEMENT un JSON array :
[
  {{"path": "memory/user.md", "append": "- Projet actif : [[Wing It]]"}},
  {{"path": "knowledge/Wing It.md", "content": "# Wing It\\nApp mobile...\\n\\n## Liens\\n- [[Figma]]\\n- [[Joris]]"}}
]

Conversation :
{conversation}
"""

_ACTION_RE = re.compile(r'ACTION:\s*(\w+)\("([^"]+)"\)')


def run_agent(messages: list[dict], on_token: Callable[[str], None] | None = None, on_memory_saved: Callable[[], None] | None = None) -> str:
    client = ollama.Client(host=OLLAMA_HOST)
    memory = get_context(messages)
    system = _SYSTEM.format(memory=memory)
    history = [{"role": "system", "content": system}] + messages

    response = ""
    for _ in range(5):
        response = ""
        for chunk in client.chat(model=MODEL, messages=history, stream=True):
            token = chunk.message.content if hasattr(chunk, "message") else chunk["message"]["content"]
            response += token
            if on_token:
                on_token(token)

        match = _ACTION_RE.search(response)
        if not match:
            break

        tool, arg = match.group(1), match.group(2)
        result = web_search(arg) if tool == "web_search" else fetch_url(arg) if tool == "fetch_url" else ""
        history.append({"role": "assistant", "content": response})
        history.append({"role": "user", "content": f"RESULT:\n{result}"})

    import threading
    def _save_and_notify():
        _save_conversation(messages, response)
        _save_memory(messages, response, client)
        if on_memory_saved:
            on_memory_saved()
    threading.Thread(target=_save_and_notify, daemon=True).start()
    return response


def _save_conversation(messages: list[dict], response: str) -> None:
    from datetime import datetime
    today = datetime.now().strftime("%Y-%m-%d")
    time_str = datetime.now().strftime("%H:%M")
    user_msg = messages[-1]["content"] if messages else ""
    # Truncate image/file blobs to just the header line
    if "\n" in user_msg and (user_msg.startswith("[Image :") or user_msg.startswith("[Fichier :")):
        user_msg = user_msg.split("\n")[0]
    entry = f"\n## {time_str}\n**Toi:** {user_msg}\n**IA:** {response}\n"
    append_note(f"conversations/{today}.md", entry)


def _save_memory(messages: list[dict], response: str, client) -> None:
    from vault.manager import list_notes
    from config import VAULT_PATH
    existing = sorted(str(p.relative_to(VAULT_PATH)) for p in list_notes())
    existing_str = "\n".join(f"- {f}" for f in existing[:60])
    conv = "\n".join(f"{m['role']}: {m['content'][:500]}" for m in messages)
    conv += f"\nassistant: {response[:500]}"
    result = client.chat(
        model=MODEL,
        messages=[{"role": "user", "content": _EXTRACT.format(
            conversation=conv[-3000:],
            existing_files=existing_str,
        )}],
    )
    text = result.message.content if hasattr(result, "message") else result["message"]["content"]
    json_match = re.search(r'\[.*\]', text, re.DOTALL)
    if not json_match:
        return
    try:
        facts = json.loads(json_match.group(), strict=False)
    except json.JSONDecodeError:
        return
    for fact in facts:
        path = fact.get("path", "")
        if not path:
            continue
        if "append" in fact:
            append_note(path, fact["append"])
        elif "content" in fact:
            write_note(path, fact["content"])
