# tests/test_indexer.py
from vault.indexer import chunk_text

def test_chunk_text_short():
    words = ["word"] * 10
    text = " ".join(words)
    chunks = chunk_text(text, size=5, overlap=1)
    assert len(chunks) == 3  # [0:5], [4:9], [8:10]

def test_chunk_text_exact_fit():
    text = " ".join(["word"] * 5)
    chunks = chunk_text(text, size=5, overlap=0)
    assert len(chunks) == 1
    assert chunks[0] == text

def test_chunk_text_empty():
    assert chunk_text("") == []

def test_chunk_text_overlap_content():
    words = ["a", "b", "c", "d", "e"]
    text = " ".join(words)
    chunks = chunk_text(text, size=3, overlap=1)
    # [a b c], [c d e]
    assert "a b c" in chunks[0]
    assert "c d e" in chunks[1]
