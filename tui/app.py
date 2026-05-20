# tui/app.py
from textual.app import App, ComposeResult
from textual.widgets import Header, Footer, TextArea, RichLog, Static
from textual.containers import Vertical
from textual.binding import Binding
from textual import work
from rich.markup import escape
from agent.core import run_agent

class PersonalAI(App):
    TITLE = "Personal AI"
    CSS = """
    #history {
        height: 1fr;
        border: solid #00ff76;
        padding: 1 2;
    }
    #streaming {
        height: auto;
        min-height: 1;
        padding: 0 2;
        color: #aaffaa;
    }
    #hint {
        height: 1;
        padding: 0 2;
        color: #555555;
    }
    #input {
        height: 4;
        border: solid #444444;
    }
    """
    BINDINGS = [
        Binding("alt+enter", "submit", "Envoyer"),
        Binding("f2", "paste_image", "Coller image"),
        Binding("escape", "quit", "Quitter"),
        Binding("ctrl+c", "quit", "Quitter"),
    ]

    def __init__(self):
        super().__init__()
        self.messages: list[dict] = []

    def compose(self) -> ComposeResult:
        yield Header(show_clock=True)
        with Vertical():
            yield RichLog(id="history", markup=True, wrap=True, highlight=True)
            yield Static("", id="streaming", markup=True)
        yield Static(
            "[dim]Alt+Entrée envoyer  /clear /memory /search <q> /note <t> /remember <t> /file <path>[/dim]",
            id="hint",
            markup=True,
        )
        yield TextArea(id="input")
        yield Footer()

    def on_mount(self) -> None:
        self.query_one("#input", TextArea).focus()
        self.query_one("#history", RichLog).write(
            "[bold green]IA personnelle prête. Dolphin-Mixtral + mémoire Obsidian.[/bold green]"
        )
        self._background_reindex()

    @work(thread=True)
    def _background_reindex(self) -> None:
        from vault.indexer import index_all
        try:
            index_all()
        except Exception:
            pass

    def action_submit(self) -> None:
        text_area = self.query_one("#input", TextArea)
        text = text_area.text.strip()
        if not text:
            return
        text_area.clear()

        # Auto-detect drag & drop (kitty pastes file path on drop)
        if "\n" not in text:
            from pathlib import Path as _Path
            candidate = _Path(text.replace("file://", "").strip()).expanduser()
            if candidate.exists() and candidate.is_file():
                self._handle_file_drop(str(candidate))
                return

        if text.startswith("/"):
            self._handle_command(text)
            return

        self.query_one("#history", RichLog).write(f"\n[bold cyan]Toi:[/bold cyan] {escape(text)}")
        self.messages.append({"role": "user", "content": text})
        self._respond()

    def action_paste_image(self) -> None:
        import subprocess, tempfile
        log = self.query_one("#history", RichLog)
        text_area = self.query_one("#input", TextArea)
        user_text = text_area.text.strip()
        text_area.clear()
        result = subprocess.run(["wl-paste", "--type", "image/png"], capture_output=True)
        if result.returncode != 0 or not result.stdout:
            stderr = result.stderr.decode().strip()
            log.write(f"[bold red]Pas d'image dans le presse-papiers.[/bold red] [dim]{escape(stderr)}[/dim]")
            return
        tmp = tempfile.NamedTemporaryFile(suffix=".png", delete=False)
        tmp.write(result.stdout)
        tmp.close()
        self._handle_file_drop(tmp.name, user_text=user_text)

    def _handle_file_drop(self, path: str, user_text: str = "") -> None:
        from pathlib import Path as _Path
        from agent.vision import is_image
        log = self.query_one("#history", RichLog)
        p = _Path(path)
        if is_image(path):
            if user_text:
                log.write(f"\n[bold cyan]Toi:[/bold cyan] {escape(user_text)}")
            log.write(f"[bold cyan]Image:[/bold cyan] {escape(p.name)} [dim](analyse en cours...)[/dim]")
            self._respond_image(path, user_text=user_text)
        else:
            from vault.indexer import index_file
            content = p.read_text(errors="replace")
            already = index_file(path, content)
            status = "[dim](déjà indexé)[/dim]" if already else "[dim](indexé ✓)[/dim]"
            log.write(f"\n[bold cyan]Fichier:[/bold cyan] {escape(p.name)} {status}")
            msg = f"[Fichier : {p.name}]\n{content}"
            if user_text:
                msg += f"\n\n{user_text}"
            self.messages.append({"role": "user", "content": msg})
            self._respond()

    @work(thread=True)
    def _respond_image(self, path: str, user_text: str = "") -> None:
        from pathlib import Path as _Path
        from agent.vision import describe_image
        log = self.query_one("#history", RichLog)
        streaming = self.query_one("#streaming", Static)
        p = _Path(path)
        try:
            self.call_from_thread(streaming.update, "[bold yellow]Analyse de l'image...[/bold yellow]")
            description = describe_image(path)
            self.call_from_thread(log.write, f"[dim]Vision : {escape(description[:200])}...[/dim]")
            content = f"[Image : {p.name}]\n{description}"
            if user_text:
                content += f"\n\nMa question : {user_text}"
            self.messages.append({"role": "user", "content": content})
        except Exception as e:
            self.call_from_thread(log.write, f"[bold red]Vision error : {escape(str(e))}[/bold red]")
            self.call_from_thread(streaming.update, "")
            return

        buffer: list[str] = []

        def on_token(token: str) -> None:
            buffer.append(token)
            self.call_from_thread(streaming.update, f"[bold green]IA:[/bold green] {escape(''.join(buffer))}")

        def on_memory_saved() -> None:
            self.call_from_thread(streaming.update, "[dim]💾 mémoire mise à jour[/dim]")
            import time; time.sleep(2)
            self.call_from_thread(streaming.update, "")

        try:
            from agent.core import run_agent
            response = run_agent(self.messages, on_token=on_token, on_memory_saved=on_memory_saved)
            self.messages.append({"role": "assistant", "content": response})
            self.call_from_thread(log.write, f"\n[bold green]IA:[/bold green] {escape(response)}")
            self.call_from_thread(streaming.update, "")
        except Exception as e:
            self.call_from_thread(log.write, f"\n[bold red]Erreur : {escape(str(e))}[/bold red]")
            self.call_from_thread(streaming.update, "")

    def _handle_command(self, cmd: str) -> None:
        log = self.query_one("#history", RichLog)
        if cmd == "/clear":
            self.messages = []
            log.clear()
            log.write("[bold green]Conversation réinitialisée.[/bold green]")
        elif cmd.startswith("/search "):
            query = cmd[8:].strip()
            from agent.memory import get_context
            ctx = get_context(query) or "Aucun résultat dans la mémoire."
            log.write(f"\n[bold yellow]Mémoire — '{query}':[/bold yellow]\n{escape(ctx[:600])}")
        elif cmd == "/memory" and self.messages:
            from agent.memory import get_context
            ctx = get_context(self.messages[-1]["content"]) or "Aucun contexte."
            log.write(f"\n[bold yellow]Contexte mémoire :[/bold yellow]\n{escape(ctx[:600])}")
        elif cmd.startswith("/note "):
            from vault.manager import append_note
            from datetime import datetime
            note_text = cmd[6:].strip()
            today = datetime.now().strftime("%Y-%m-%d")
            append_note(f"conversations/{today}.md", f"- {note_text}")
            log.write("[bold green]Note enregistrée.[/bold green]")
        elif cmd.startswith("/remember "):
            from vault.manager import append_note
            content = cmd[10:].strip()
            append_note("memory/user.md", content)
            log.write("[bold green]Mémoire mise à jour directement.[/bold green]")
        elif cmd == "/paste":
            self.action_paste_image()
        elif cmd == "/reindex":
            log.write("[dim]Réindexation de la vault...[/dim]")
            self._reindex()
        elif cmd.startswith("/consolidate"):
            days = 1
            parts = cmd.split()
            if len(parts) > 1 and parts[1].isdigit():
                days = int(parts[1])
            log.write(f"[dim]Consolidation des {days} dernier(s) jour(s)...[/dim]")
            self._consolidate(days)
        elif cmd.startswith("/file "):
            from pathlib import Path
            path = Path(cmd[6:].strip()).expanduser()
            if not path.exists():
                log.write(f"[bold red]Fichier introuvable : {path}[/bold red]")
                return
            self._handle_file_drop(str(path))

    @work(thread=True)
    def _consolidate(self, days: int = 1) -> None:
        from agent.consolidate import consolidate
        log = self.query_one("#history", RichLog)
        try:
            msg = consolidate(days_back=days)
            self.call_from_thread(log.write, f"[bold green]{escape(msg)}[/bold green]")
        except Exception as e:
            self.call_from_thread(log.write, f"[bold red]Erreur consolidation : {escape(str(e))}[/bold red]")

    @work(thread=True)
    def _reindex(self) -> None:
        from vault.indexer import index_all
        log = self.query_one("#history", RichLog)
        try:
            index_all()
            self.call_from_thread(log.write, "[bold green]Vault réindexée.[/bold green]")
        except Exception as e:
            self.call_from_thread(log.write, f"[bold red]Erreur reindex : {escape(str(e))}[/bold red]")

    @work(thread=True)
    def _respond(self) -> None:
        log = self.query_one("#history", RichLog)
        streaming = self.query_one("#streaming", Static)
        buffer: list[str] = []

        self.call_from_thread(streaming.update, "[bold yellow]Réflexion en cours...[/bold yellow]")

        def on_token(token: str) -> None:
            buffer.append(token)
            self.call_from_thread(
                streaming.update,
                f"[bold green]IA:[/bold green] {escape(''.join(buffer))}"
            )

        def on_memory_saved() -> None:
            self.call_from_thread(streaming.update, "[dim]💾 mémoire mise à jour[/dim]")
            import time; time.sleep(2)
            self.call_from_thread(streaming.update, "")

        try:
            response = run_agent(self.messages, on_token=on_token, on_memory_saved=on_memory_saved)
            self.messages.append({"role": "assistant", "content": response})
            self.call_from_thread(log.write, f"\n[bold green]IA:[/bold green] {escape(response)}")
            self.call_from_thread(streaming.update, "")
        except Exception as e:
            self.call_from_thread(log.write, f"\n[bold red]Erreur : {escape(str(e))}[/bold red]")
            self.call_from_thread(streaming.update, "")
