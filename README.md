# pitwall

> Your pit wall for Claude Code sessions.
> A lightweight control tower that sits next to your IDE: see every session across every project, approve and reply from one screen, and keep each project's memory in view.

**Status:** pre-alpha, design stage. Nothing to install yet.

---

## Why

Running several Claude Code sessions across several projects gets messy quickly:

- You have 3 IDE windows with 3–4 terminals each, and you keep tab-hopping to find out **who is waiting for you**.
- A session sits blocked on a permission prompt or a question for 20 minutes because you didn't notice.
- Each project's `CLAUDE.md` and `.claude/` rules change as you work, and there's no single place to see them.

pitwall is a pit wall for that: one screen to watch every session, answer the ones that need you, and keep moving.

## What it does (planned)

- **One board for all sessions**: sessions grouped by project, with live status (working / needs you / idle / done / failed).
- **"Needs you" inbox**: every session waiting on a permission or a question, in one list, plus OS notifications.
- **Approve and reply in place**: allow or deny permission requests and answer questions without switching windows.
- **Launch sessions where they belong**: pick a project, choose *new worktree / existing worktree / main folder*, and pitwall starts `claude` in that directory.
- **Project memory panel**: view and edit `CLAUDE.md` and `.claude/` rules per project, and see when a session changed them.
- **Project status**: branch, worktrees, uncommitted changes, recent commits.

## How it differs from other tools

| | pitwall | Full agent IDEs (e.g. Orca) |
|---|---|---|
| Where you write code | Your existing IDE (IntelliJ, VS Code, …) | Inside the agent IDE |
| Default permissions | **Safe by default**: Claude Code's own permission rules, approvals surfaced on the board | Often full autonomy by default |
| Scope | Claude Code only, deeply integrated through official hooks | Many agent CLIs |
| Footprint | Single small binary, runs on localhost | Full desktop IDE |
| Platform | Windows, macOS and Linux, Windows treated as a first-class target | Varies |

pitwall deliberately does **not** do diff review, merging, or code editing. Your IDE already does those well.

## How it works

```
Claude Code sessions ──(official hooks)──▶ pitwall ──▶ web UI on localhost
                    ◀──(PTY, for sessions pitwall launched)──┘
```

- **Observed sessions**: sessions you start yourself (for example in an IDE terminal) report their status to pitwall through Claude Code hooks. pitwall can show their status and answer permission requests through the `PermissionRequest` hook.
- **Owned sessions**: sessions launched from pitwall run in a pseudo-terminal that pitwall owns, so you can type into them directly from the board.

See [docs/DESIGN.md](docs/DESIGN.md) for the full design.

## Planned install

```bash
npx pitwall-cli        # or: brew / scoop / winget / GitHub Releases
pitwall init           # installs Claude Code hooks (backs up your settings first)
pitwall                # starts the board and opens your browser
```

## Security model

pitwall can start processes and answer permission prompts, so it is built to be boring and safe:

- Listens on `127.0.0.1` only. Never on a public interface.
- Every request needs a random token generated at startup. The browser gets it through the launch URL; hooks read it from `~/.pitwall/token`.
- `pitwall init` never overwrites your Claude Code settings. It backs them up, shows you the diff, and merges.
- No telemetry.

## Tech stack

- **Core and CLI:** Rust (tokio, axum, portable-pty)
- **UI:** TypeScript, React, Vite, xterm.js
- **Desktop app (later):** Tauri, reusing the same core and UI

## Roadmap

- [x] M0: Repository skeleton, CI
- [ ] M1: Hook receiver and read-only board for observed sessions
- [ ] M2: Owned sessions (PTY) with in-browser terminals and replies
- [ ] M3: Approvals from the board, OS notifications
- [ ] M4: Project memory and git status panels
- [ ] Later: `npx` distribution, Tauri desktop app (tray badge, global shortcut)

## Not affiliated with Anthropic

pitwall is an independent open-source project. "Claude" and "Claude Code" are trademarks of Anthropic. pitwall works with Claude Code through its public hooks and CLI.

## License

MIT
