# main.py
from vault.indexer import index_all
from tui.app import PersonalAI

def main():
    print("Indexation de la vault...", flush=True)
    index_all()
    PersonalAI().run()

if __name__ == "__main__":
    main()
