# vault/indexer.py
import hashlib
import ollama
import chromadb
from pathlib import Path
from config import VAULT_PATH, CHROMA_PATH, EMBED_MODEL, OLLAMA_HOST, CHUNK_SIZE, CHUNK_OVERLAP, TOP_K

def _content_hash(content: str) -> str:
    return hashlib.sha256(content.encode()).hexdigest()[:16]

def chunk_text(text: str, size: int = CHUNK_SIZE, overlap: int = CHUNK_OVERLAP) -> list[str]:
    words = text.split()
    if not words:
        return []
    chunks = []
    step = size - overlap
    for i in range(0, len(words), step):
        chunks.append(" ".join(words[i:i + size]))
        if i + size >= len(words):
            break
    return chunks

def _get_collection():
    CHROMA_PATH.mkdir(parents=True, exist_ok=True)
    client = chromadb.PersistentClient(path=str(CHROMA_PATH))
    return client.get_or_create_collection("vault")

def _embed(texts: list[str]) -> list[list[float]]:
    client = ollama.Client(host=OLLAMA_HOST)
    return [client.embeddings(model=EMBED_MODEL, prompt=t)["embedding"] for t in texts]

def index_note(path: Path) -> None:
    content = path.read_text(encoding="utf-8")
    chunks = chunk_text(content)
    if not chunks:
        return
    col = _get_collection()
    rel = str(path.relative_to(VAULT_PATH))
    embeddings = _embed(chunks)
    ids = [f"{rel}::{i}" for i in range(len(chunks))]
    col.upsert(
        ids=ids,
        embeddings=embeddings,
        documents=chunks,
        metadatas=[{"path": rel} for _ in chunks],
    )

def index_file(filepath: str, content: str) -> bool:
    """Index an arbitrary file into ChromaDB. Returns True if already indexed."""
    h = _content_hash(content)
    col = _get_collection()
    existing = col.get(where={"hash": h}, limit=1)
    if existing["ids"]:
        return True
    chunks = chunk_text(content)
    if not chunks:
        return False
    embeddings = _embed(chunks)
    ids = [f"import:{h}:{i}" for i in range(len(chunks))]
    col.add(
        ids=ids,
        embeddings=embeddings,
        documents=chunks,
        metadatas=[{"path": filepath, "hash": h} for _ in chunks],
    )
    return False

def index_all() -> None:
    for note in VAULT_PATH.rglob("*.md"):
        index_note(note)

def search(query: str, top_k: int = TOP_K) -> list[dict]:
    col = _get_collection()
    emb = _embed([query])[0]
    results = col.query(query_embeddings=[emb], n_results=top_k)
    if not results["documents"][0]:
        return []
    return [
        {"path": m["path"], "content": d}
        for m, d in zip(results["metadatas"][0], results["documents"][0])
    ]
