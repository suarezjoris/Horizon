# agent/consolidate.py
import re
import json
from datetime import datetime, timedelta
import ollama
from config import VAULT_PATH, MODEL, OLLAMA_HOST
from vault.manager import write_note, append_note, list_notes

_CONSOLIDATE_PROMPT = """\
Tu es un système de mémoire neuronal autonome. Analyse les conversations ci-dessous et construis le graphe de connaissance de Joris.

FICHIERS EXISTANTS dans la vault :
{existing_files}

OBJECTIF :
1. Extrais tous les faits durables : projets, décisions, préférences, découvertes, états d'avancement
2. Déduis des connexions implicites (ex: "Code Interpreter en Python" + "veut créer un OS" → [[Code-Interpreter]] influence [[OS custom]])
3. Crée/mets à jour des pages knowledge/ pour chaque sujet significatif avec [[wikilinks]]
4. Mets à jour memory/user.md uniquement avec des NOUVEAUX faits non déjà présents
5. Relie tout ce qui peut l'être

RÈGLES :
- Utilise [[wikilinks]] partout pour créer le graphe
- Une page knowledge/ = un sujet (projet, outil, concept, personne)
- memory/user.md = faits atomiques sur Joris uniquement
- INTERDIT : coller des transcriptions, dialogues, résumés verbatim
- Si rien de nouveau → retourne []

Retourne UNIQUEMENT un JSON array :
[
  {{"path": "memory/user.md", "append": "- [[Cyberdeck]] bientôt terminé (Compaq Presario M2000)"}},
  {{"path": "knowledge/Cyberdeck.md", "content": "# Cyberdeck\\nProjet de Joris : transformer un [[Compaq Presario M2000]] en cyberdeck.\\n\\n## Liens\\n- [[Joris]]\\n- [[Code-Interpreter]]"}},
]

Conversations à analyser :
{conversations}
"""


def consolidate(days_back: int = 1) -> str:
    today = datetime.now()
    conv_texts: list[str] = []

    for d in range(days_back + 1):
        date = (today - timedelta(days=d)).strftime("%Y-%m-%d")
        path = VAULT_PATH / f"conversations/{date}.md"
        if path.exists():
            conv_texts.append(f"## {date}\n{path.read_text(encoding='utf-8')}")

    if not conv_texts:
        return "Aucune conversation à analyser."

    existing = sorted(str(p.relative_to(VAULT_PATH)) for p in list_notes())
    existing_str = "\n".join(f"- {f}" for f in existing[:80])
    full_conv = "\n\n---\n\n".join(conv_texts)

    client = ollama.Client(host=OLLAMA_HOST)
    result = client.chat(
        model=MODEL,
        messages=[{"role": "user", "content": _CONSOLIDATE_PROMPT.format(
            conversations=full_conv[-6000:],
            existing_files=existing_str,
        )}],
    )
    text = result.message.content if hasattr(result, "message") else result["message"]["content"]

    json_match = re.search(r'\[.*\]', text, re.DOTALL)
    if not json_match:
        return f"Aucun fait extrait. Réponse LLM : {text[:200]}"

    try:
        facts = json.loads(json_match.group(), strict=False)
    except json.JSONDecodeError as e:
        return f"Erreur JSON : {e} — extrait : {json_match.group()[:200]}"

    saved = 0
    for fact in facts:
        path = fact.get("path", "")
        if not path:
            continue
        if "append" in fact:
            append_note(path, fact["append"])
            saved += 1
        elif "content" in fact:
            write_note(path, fact["content"])
            saved += 1

    return f"{saved} faits consolidés dans la vault."


if __name__ == "__main__":
    print(consolidate())
