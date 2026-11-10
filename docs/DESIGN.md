# pitwall 설계 문서

- 상태: 초안 v0.1
- 작성일: 2026-11-10
- 대상 독자: pitwall을 구현하는 사람과 Claude Code

이 문서는 구현의 기준이다. 구현 중 이 문서와 다른 결정을 내려야 하면, 코드를 바꾸기 전에 이 문서의 해당 부분을 먼저 고치고 이유를 "결정 기록"에 남긴다.

---

## 1. 배경과 목표

### 1.1 문제
- 프로젝트 3개, 프로젝트마다 Claude Code 세션 3~4개를 IDE 터미널에서 동시에 돌린다.
- 어느 세션이 입력이나 권한 승인을 기다리는지 알려면 IDE 창과 탭을 하나씩 열어 봐야 한다.
- 세션들이 프로젝트 메모리(`CLAUDE.md`, `.claude/`)를 갱신하는데, 프로젝트별로 한눈에 볼 곳이 없다.

### 1.2 목표
1. 모든 프로젝트의 모든 세션 상태를 한 화면에서 본다.
2. 나를 기다리는 세션(권한 요청, 질문)을 모아 보고, 그 자리에서 승인하거나 답한다.
3. 상황판에서 프로젝트와 작업 위치(worktree)를 골라 새 세션을 띄운다.
4. 프로젝트별 메모리와 git 상태를 본다.
5. 다른 개발자가 쉽게 설치할 수 있도록 가볍게 배포한다. 단일 바이너리, Windows 우선 지원.

### 1.3 하지 않는 것 (Non-goals)
- 코드 편집기, diff 리뷰, 머지, PR 생성. IDE와 GitHub가 한다.
- Claude Code 외 다른 에이전트 CLI 지원. 범위가 넓어지면 Orca 같은 도구와 정면으로 비교된다.
- 원격 접속, 팀 공유, 클라우드 동기화. 로컬 전용이다.
- Claude Code의 내부 파일(`~/.claude/projects/**/*.jsonl`) 형식에 의존하는 기능. 비공식 형식이라 깨질 수 있다. 꼭 필요하면 "선택 기능"으로만 둔다.

### 1.4 포지셔닝
"IDE 옆에 띄워 두는 Claude Code 관제탑. 안전한 승인, 한 화면 응답, 프로젝트 메모리 관리."
- 권한은 Claude Code 기본 규칙을 따른다. `--dangerously-skip-permissions`를 기본으로 켜지 않는다.

---

## 2. 핵심 개념

| 용어 | 정의 |
|---|---|
| Project | 사용자가 등록한 git 저장소 루트 폴더. 예: `~/work/cdp-backend` |
| Workspace | 세션이 실행되는 실제 폴더. 프로젝트 메인 폴더 또는 그 프로젝트의 git worktree |
| Session | Claude Code 프로세스 하나. Claude Code의 `session_id`로 식별 |
| Observed session | 사용자가 직접(예: IntelliJ 터미널) 띄운 세션. hook으로만 상태를 받는다 |
| Owned session | pitwall이 PTY로 직접 띄운 세션. 입출력을 pitwall이 가진다 |
| Pending request | 세션이 사용자에게 요청한 것. 권한 요청(permission) 또는 질문(question) |

### 2.1 세션 상태

```
starting ──▶ working ◀──▶ needs_permission
               │  ▲          
               │  └──────▶ needs_input
               ▼
             idle ──▶ working
               │
               ▼
     ended / failed / stopped
```

| 상태 | 의미 | 들어오는 계기 |
|---|---|---|
| `starting` | 세션이 등록됐고 아직 첫 지시가 없음 | `SessionStart` hook, 또는 owned 세션 spawn |
| `working` | Claude가 처리 중 | `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, 권한 응답 직후 |
| `needs_permission` | 도구 사용 승인 대기 | `PermissionRequest` hook |
| `needs_input` | Claude가 사용자 입력을 기다림 (질문 등) | `Notification` hook (메시지로 판별), `Stop` 이후 질문으로 끝난 경우는 판별하지 않음 |
| `idle` | 응답을 끝내고 다음 지시를 기다림 | `Stop` hook |
| `ended` | 세션 정상 종료 | `SessionEnd` hook, owned 프로세스 정상 종료 |
| `failed` | 비정상 종료 | owned 프로세스 비정상 종료 코드 |
| `stopped` | 사용자가 pitwall에서 중지 | 중지 API |

추가 규칙:
- observed 세션에서 hook이 일정 시간(기본 30분) 동안 오지 않으면 `stale` 표시를 붙인다. 상태 자체는 바꾸지 않는다.
- 상태 전환은 `pitwall-core`의 한 곳(상태 머신 모듈)에서만 일어난다. 테스트로 전환표를 고정한다.

---

## 3. 전체 구조

```
┌──────────────────────── 사용자 PC ────────────────────────┐
│                                                           │
│  IntelliJ 터미널의 claude ──hook──┐                         │
│                                  ▼                         │
│  pitwall hook <event> ──HTTP(token)──▶ ┌──────────────┐    │
│  (Claude Code가 실행하는 명령)          │ pitwall 서버   │    │
│                                        │ 127.0.0.1     │    │
│  pitwall이 띄운 claude ◀──PTY──────────▶│ (axum+tokio)  │    │
│                                        └──────┬───────┘    │
│                                   REST + WebSocket          │
│                                               ▼            │
│                                     브라우저 (React UI)      │
└───────────────────────────────────────────────────────────┘
```

### 3.1 저장소 구조

```
pitwall/
├─ Cargo.toml                 # workspace
├─ crates/
│  ├─ pitwall-core/           # 도메인 로직. HTTP를 모른다
│  │  ├─ project.rs           # 프로젝트 등록, 설정 파일
│  │  ├─ session/             # 세션 모델, 상태 머신, 레지스트리
│  │  ├─ hooks/               # hook 이벤트 파싱, 상태 반영
│  │  ├─ permission.rs        # 대기 중인 권한 요청과 응답 대기(oneshot)
│  │  ├─ pty.rs               # owned 세션 spawn, 입출력 스트림
│  │  ├─ memory.rs            # CLAUDE.md, .claude/ 읽기·쓰기·감시
│  │  ├─ git.rs               # git 명령 실행, worktree 생성
│  │  └─ events.rs            # 내부 이벤트 버스 (tokio broadcast)
│  └─ pitwall-cli/            # 실행 파일 `pitwall`
│     ├─ main.rs              # clap: serve(기본), init, hook, doctor
│     ├─ server/              # axum 라우터, REST, WebSocket, 토큰 미들웨어
│     └─ embed.rs             # rust-embed로 web/dist 내장
├─ web/                       # React + TypeScript + Vite + xterm.js
├─ desktop/                   # (M5 이후) Tauri
├─ npm/                       # (M5 이후) npx 설치 래퍼
└─ docs/
   └─ DESIGN.md
```

원칙: `pitwall-core`는 axum이나 HTTP 타입에 의존하지 않는다. 나중에 Tauri 앱이 같은 코어를 그대로 쓴다.

### 3.2 기술 선택

| 영역 | 선택 | 비고 |
|---|---|---|
| 비동기 런타임 | tokio | |
| HTTP, WebSocket | axum | |
| PTY | portable-pty | Windows는 ConPTY |
| 파일 감시 | notify | |
| 직렬화 | serde, serde_json | |
| CLI | clap | |
| UI 내장 | rust-embed | 릴리스 빌드에서 `web/dist` 포함 |
| 로깅 | tracing | |
| git | `git` 명령 실행 | `git2`는 쓰지 않는다. worktree 지원과 사용자 git 설정을 그대로 따르기 위해 |
| 프론트 | React, TypeScript, Vite | 상태 관리는 가볍게 (zustand 정도) |
| 터미널 렌더링 | xterm.js + fit addon | |

---

## 4. Claude Code 연동

> 구현 전에 반드시 Claude Code 공식 hooks 문서(https://code.claude.com/docs)에서 이벤트 이름, 입력 JSON 필드, 출력 형식, 타임아웃 기본값을 다시 확인한다. 아래 내용은 설계 시점 기준이며 확인되지 않은 부분은 [확인 필요]로 표시했다.

### 4.1 hook 설치 (`pitwall init`)
- 대상: 사용자 전역 설정 `~/.claude/settings.json`. 프로젝트별 설정은 건드리지 않는다.
- 순서:
  1. 기존 파일을 `~/.claude/settings.json.pitwall-backup-<timestamp>`로 백업한다.
  2. 추가할 hook을 기존 설정과 **병합**한다. 기존 hook은 지우거나 바꾸지 않는다.
  3. 변경 diff를 보여 주고 확인(y/N)을 받은 뒤 쓴다. `--yes`로 생략 가능.
  4. `pitwall init --uninstall`은 pitwall이 추가한 항목만 제거한다. 식별을 위해 명령 문자열에 `pitwall hook`이 들어간 항목만 대상으로 한다.
- 등록할 이벤트: `SessionStart`, `SessionEnd`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `Notification`, `Stop`, `PermissionRequest`
- 각 hook의 명령: `pitwall hook <EventName>`

### 4.2 `pitwall hook <event>` 서브커맨드
curl 대신 자체 서브커맨드를 쓴다. Windows와 macOS에서 똑같이 동작하고, 토큰 처리와 실패 처리를 한 곳에서 할 수 있다.

- stdin으로 받은 hook JSON을 그대로 `POST http://127.0.0.1:<port>/hooks/<event>`로 보낸다.
- 포트와 토큰은 `~/.pitwall/runtime.json`에서 읽는다 (서버가 시작할 때 쓴다).
- **서버가 꺼져 있거나 응답이 없으면 아무것도 출력하지 않고 exit 0.** Claude Code 작업을 절대 막지 않는다.
- 일반 이벤트는 짧은 타임아웃(1초)으로 보내고 응답을 기다리지 않는다.
- `PermissionRequest`만 응답을 기다린다 (4.4).

### 4.3 이벤트별 처리

| hook | 사용하는 필드 | 처리 |
|---|---|---|
| `SessionStart` | `session_id`, `cwd`, `source` [확인 필요] | 세션 등록. `cwd`로 프로젝트와 workspace 매칭. 상태 `starting` |
| `UserPromptSubmit` | `session_id`, `prompt` | 상태 `working`, 마지막 지시 저장 |
| `PreToolUse` | `session_id`, `tool_name`, `tool_input` | 상태 `working`, "현재 작업" 표시 (예: `Bash: ./gradlew test`). 서버에서 디바운스 |
| `PostToolUse` | `session_id`, `tool_name` | 활동 로그에 추가 |
| `Notification` | `session_id`, `message` | 메시지가 입력 대기를 뜻하면 `needs_input`. 알림 발송 |
| `PermissionRequest` | `session_id`, `tool_name`, `tool_input` | `needs_permission`, 대기 요청 생성 (4.4) |
| `Stop` | `session_id` | 상태 `idle` |
| `SessionEnd` | `session_id`, `reason` [확인 필요] | 상태 `ended` |

- `cwd` 매칭 규칙: 등록된 프로젝트 경로 또는 그 프로젝트의 worktree 경로(`git worktree list --porcelain`으로 구함) 아래면 그 프로젝트 소속이다. 어디에도 안 맞으면 "미분류" 프로젝트로 보여 주고, UI에서 등록을 제안한다.
- 경로 비교는 정규화(대소문자, 구분자, 심볼릭 링크) 후 한다. Windows에서 특히 주의한다.

### 4.4 권한 승인 흐름 (핵심 기능)

observed 세션과 owned 세션 모두 같은 방식으로 처리한다.

```
Claude Code ─▶ pitwall hook PermissionRequest ─▶ POST /hooks/PermissionRequest
                                                   │ 대기 요청 생성, UI로 이벤트
                                                   │ 사용자 응답 또는 타임아웃까지 대기
사용자 ─▶ UI [허용] / [거부] ─▶ POST /api/sessions/:id/permission
                                                   │
          hook이 결정을 stdout으로 출력 ◀────────────┘
```

- 서버는 요청마다 `tokio::sync::oneshot`을 만들고, UI 응답이 오면 결정을 보낸다.
- hook 출력 형식 [확인 필요]:
  ```json
  {"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}
  ```
  거부: `"behavior":"deny"`와 사유 메시지.
- **타임아웃 동작**: 정해진 시간(기본 50초. Claude Code hook 타임아웃보다 짧게) 안에 응답이 없으면 **아무 결정도 출력하지 않고 exit 0** 한다. 그러면 Claude Code가 원래대로 터미널에서 사용자에게 묻는다. pitwall이 대신 허용하는 일은 절대 없다.
- 터미널에서 먼저 답한 경우: 대기 요청이 남아 있을 수 있으므로, 같은 세션에서 `PostToolUse`나 `UserPromptSubmit`, `Stop`이 오면 그 세션의 대기 요청을 정리한다.
- "항상 허용" 같은 규칙 저장 기능은 MVP에서 하지 않는다. 그건 Claude Code의 permissions 설정이 할 일이다.

### 4.5 owned 세션 (PTY)
- spawn: `portable-pty`로 `claude`를 실행한다. `cwd`는 workspace 경로.
- 첫 지시가 있으면 프로세스가 준비된 뒤 PTY에 입력한다. 또는 `claude "<prompt>"` 인자로 넘기는 방식을 검토한다 [확인 필요].
- 출력은 바이트 스트림 그대로 WebSocket으로 보내고, 브라우저의 xterm.js가 그린다. 서버는 출력을 해석하지 않는다.
- 서버는 세션마다 최근 출력 일부(예: 256KB 링버퍼)를 보관해서, 브라우저를 새로 열어도 화면을 복원한다.
- 터미널 크기 변경(resize)을 지원한다.
- owned 세션도 hook을 통해 상태를 받는다. 즉 상태 판단 경로는 하나뿐이다. PTY는 입출력 전달만 한다.
- owned 세션과 hook 이벤트 연결: spawn할 때 환경 변수 `PITWALL_SESSION_TAG=<uuid>`를 넣고, `pitwall hook`이 이 값을 함께 보낸다. 서버는 첫 hook에서 `session_id`와 tag를 묶는다.

### 4.6 새 세션 만들기
입력: 프로젝트, 작업 위치, 브랜치 이름, 첫 지시(선택)

| 작업 위치 | 동작 |
|---|---|
| 새 worktree | `git -C <project> worktree add <dir> -b <branch>`. `<dir>` 기본값은 `<project>-<branch 마지막 조각>`. 이미 있으면 오류 |
| 기존 worktree | 선택한 worktree 경로에서 실행 |
| 메인 폴더 | 프로젝트 루트에서 실행. UI에 "다른 세션과 파일 충돌 주의" 경고 |

- 브랜치 이름 검증: 비어 있으면 안 되고, 공백이 들어가면 안 되며, `git check-ref-format --branch`를 통과해야 한다.
- worktree 생성이 실패하면 세션을 만들지 않고 git 오류 메시지를 그대로 보여 준다.

---

## 5. 데이터와 저장

MVP에는 DB를 두지 않는다.

| 파일 | 내용 |
|---|---|
| `~/.pitwall/config.toml` | 등록한 프로젝트 목록, 포트, 알림 설정 |
| `~/.pitwall/runtime.json` | 실행 중인 서버의 포트, 토큰, PID. 서버 종료 시 삭제. 권한 0600 (Windows는 사용자 전용 ACL) |
| 메모리 상태 | 세션 목록과 상태, 대기 요청, 활동 로그. 서버를 재시작하면 사라진다 |

`config.toml` 예시:
```toml
port = 7311

[[projects]]
name = "cdp-backend"
path = "C:/work/cdp-backend"

[notifications]
needs_permission = true
needs_input = true
done = false
```

### 5.1 핵심 타입 (Rust 기준 개요)
```rust
struct Project { id: ProjectId, name: String, path: PathBuf }

struct Session {
    id: SessionId,                // Claude Code session_id
    project_id: Option<ProjectId>, // None이면 미분류
    workspace: PathBuf,
    branch: Option<String>,
    origin: Origin,               // Observed | Owned { tag: Uuid }
    status: Status,
    current_activity: Option<String>, // 예: "Bash: ./gradlew test"
    last_prompt: Option<String>,
    pending: Option<PendingRequest>,
    started_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

enum PendingRequest {
    Permission { request_id: Uuid, tool_name: String, summary: String },
    Input { message: String },
}
```

---

## 6. API

모든 요청에 토큰이 필요하다 (7장). 응답은 JSON.

### 6.1 REST
| 메서드 | 경로 | 설명 |
|---|---|---|
| GET | `/api/projects` | 프로젝트 목록과 세션 수, 대기 수 |
| POST | `/api/projects` | 프로젝트 등록 `{ path }` |
| DELETE | `/api/projects/:id` | 등록 해제 (파일은 건드리지 않음) |
| GET | `/api/sessions` | 전체 세션. `?project=` 필터 |
| POST | `/api/sessions` | owned 세션 생성 `{ project_id, location, branch?, worktree?, prompt? }` |
| POST | `/api/sessions/:id/stop` | owned 세션 중지. observed는 400 |
| POST | `/api/sessions/:id/permission` | `{ request_id, decision: "allow" \| "deny", message? }` |
| GET | `/api/projects/:id/memory` | `CLAUDE.md`, `CLAUDE.local.md`, `.claude/` 아래 텍스트 파일 목록과 내용 |
| PUT | `/api/projects/:id/memory` | `{ path, content, base_hash }`. 충돌 시 409 |
| GET | `/api/projects/:id/git` | 현재 브랜치, worktree 목록, 변경 파일 수, 최근 커밋 10개 |
| POST | `/hooks/:event` | `pitwall hook` 전용 |

### 6.2 WebSocket `/ws`
서버 → 클라이언트:
- `session.upserted` (세션 전체 스냅샷)
- `session.removed`
- `request.created`, `request.resolved`
- `memory.changed` `{ project_id, path }`
- `pty.output` `{ session_id, data(base64) }`

클라이언트 → 서버:
- `pty.input` `{ session_id, data }`
- `pty.resize` `{ session_id, cols, rows }`
- `pty.attach` `{ session_id }` (링버퍼 내용 먼저 받기)

---

## 7. 보안

pitwall은 프로세스를 실행하고 권한 요청에 답할 수 있다. 다음은 필수 사항이다.

1. `127.0.0.1`에만 바인딩한다. `0.0.0.0` 옵션은 만들지 않는다.
2. 서버 시작 시 256비트 랜덤 토큰을 만든다.
   - 브라우저: 시작 URL `http://127.0.0.1:7311/#token=...`로 전달한다. 프론트는 이를 메모리에 보관하고 `Authorization: Bearer` 헤더로 보낸다. 쿠키는 쓰지 않는다.
   - hook: `~/.pitwall/runtime.json`에서 읽는다.
3. `Origin` 헤더 검사: REST와 WebSocket 모두 `http://127.0.0.1:<port>`만 허용한다. 다른 웹사이트가 localhost로 요청을 보내는 공격(DNS rebinding, CSRF)을 막는다. `Host` 헤더도 검사한다.
4. 메모리 편집 API는 등록된 프로젝트 경로 아래의 허용된 파일(`CLAUDE.md`, `CLAUDE.local.md`, `.claude/**`의 텍스트 파일)만 다룬다. `..` 같은 경로 탈출을 막는다.
5. 서버 로그에 프롬프트 내용과 도구 입력을 기본으로 남기지 않는다.
6. 텔레메트리 없음.

---

## 8. 화면 (UI)

목업 기준: 3단 레이아웃.

- **상단 바**: 입력 대기, 작업 중, 완료 개수. [+ 새 세션]
- **왼쪽**: 프로젝트 목록(세션 수, 대기 배지), "나를 기다리는 세션" 목록, [+ 폴더 추가]
- **가운데**: 세션 카드 그리드. 카드는 이름, 프로젝트, 브랜치, 상태, 최근 활동을 보여 준다.
  - `needs_permission`: 도구와 명령 요약, [허용] [거부]
  - `needs_input`: Claude 메시지. owned 세션이면 답장 입력칸, observed 세션이면 "IDE 터미널에서 답하세요" 안내
  - 카드 제목을 누르면 크게 보기. owned 세션은 xterm.js 터미널이 열린다.
- **크게 보기**: 위쪽에 다른 세션으로 바로 가는 버튼 줄, [← 전체 보기], [세션 중지]
- **오른쪽 패널**: 선택한 프로젝트의 [메모리] [Git] 탭
- **새 세션 창**: 프로젝트, 작업 위치(새 worktree / 기존 worktree / 메인 폴더), 브랜치, 첫 지시, 실행될 명령 미리보기
- **알림**: 브라우저 Notification API. `needs_permission`, `needs_input` 때. 탭 제목에 대기 수 표시 (`(2) pitwall`)

UI 원칙:
- 상태는 색만으로 구분하지 않는다. 라벨 텍스트와 아이콘을 같이 쓴다.
- 키보드만으로 대기 세션 사이를 이동하고 승인할 수 있게 한다 (M3).

---

## 9. 마일스톤

각 마일스톤은 "완료 기준"을 모두 통과해야 끝난다.

### M0. 저장소 골격
- Cargo workspace(`pitwall-core`, `pitwall-cli`), `web/` Vite 프로젝트
- `cargo fmt`, `cargo clippy -D warnings`, `cargo test`, 프론트 `lint`·`typecheck`·`test`가 도는 GitHub Actions (Windows, macOS, Linux)
- 완료 기준: `cargo run -p pitwall-cli`가 빈 페이지를 내장 UI로 띄운다. CI 3개 OS 통과

### M1. hook 수신 + 읽기 전용 보드
- `pitwall init` (백업, 병합, diff 확인, uninstall)
- `pitwall hook <event>`
- 서버: 토큰, Origin 검사, `/hooks/*`, 상태 머신, `/api/projects`, `/api/sessions`, WebSocket `session.*`
- UI: 프로젝트 목록, 세션 카드, 상태 배지, 대기 목록
- 완료 기준: IntelliJ(또는 아무 터미널)에서 띄운 claude 세션 3개가 보드에 나타나고, 작업 중 → 대기 → 응답 완료 상태가 실시간으로 바뀐다. 서버를 끈 상태에서도 claude가 정상 동작한다

### M2. owned 세션 (PTY)
- `POST /api/sessions`, worktree 생성, PTY spawn, 링버퍼, `pty.*` WebSocket
- UI: 새 세션 창, 크게 보기 xterm.js 터미널, 세션 중지
- 완료 기준: 상황판에서 새 worktree로 세션을 만들고, 브라우저 터미널에서 대화하고, 새로고침 후에도 화면이 복원된다. Windows에서 동작한다

### M3. 승인과 알림
- `PermissionRequest` 대기와 응답, 타임아웃 시 터미널로 넘기기, 대기 요청 정리
- UI: 카드와 크게 보기의 [허용] [거부], 브라우저 알림, 탭 제목 배지, 키보드 단축키
- 완료 기준: observed 세션의 권한 요청을 보드에서 허용하면 Claude가 계속 진행한다. 50초 동안 응답하지 않으면 터미널에 원래 권한 프롬프트가 뜬다

### M4. 메모리와 git 패널
- 메모리 파일 목록, 보기, 편집(충돌 검사), `notify`로 변경 감지
- git 요약 (`git status --porcelain`, `git worktree list --porcelain`, `git log`)
- 완료 기준: 세션이 `CLAUDE.md`를 고치면 패널에 "방금 변경됨"이 뜬다. 보드에서 편집해 저장하면 파일에 반영된다

### M5 이후 (범위 밖, 메모만)
- 배포: GitHub Releases 바이너리, `npx pitwall-cli`, scoop, winget, Homebrew
- Tauri 앱: 트레이 배지, OS 알림, 전역 단축키
- 선택 기능: 세션 기록 검색 (공식 데이터만 사용)

---

## 10. 테스트 전략
- `pitwall-core`
  - 상태 머신: 이벤트 순서별 전환표 테스트
  - hook JSON 파싱: 실제 hook 페이로드 예시를 `tests/fixtures/`에 저장하고 파싱 테스트. 모르는 필드가 와도 깨지지 않아야 한다 (`#[serde(default)]`, 알 수 없는 필드 허용)
  - 권한 대기: 응답, 타임아웃, 중복 응답, 터미널에서 먼저 답한 경우
  - cwd 매칭: Windows 경로, 대소문자, worktree 경로
  - 메모리 경로 검증: 경로 탈출 시도 거부
- `pitwall-cli`
  - 토큰 없는 요청 401, 잘못된 Origin 403
  - `init` 병합: 기존 hook이 있는 settings.json, hook이 없는 경우, 파일이 없는 경우, 망가진 JSON인 경우(쓰지 않고 오류)
- 프론트: 상태별 카드 렌더링, 승인 버튼 동작, WebSocket 재연결
- 수동 시나리오 체크리스트: `docs/manual-test.md`에 마일스톤별로 정리

---

## 11. 위험과 대응

| 위험 | 대응 |
|---|---|
| Claude Code hook 형식이 바뀜 | 필드를 관대하게 파싱. fixture 테스트. 실패해도 claude를 막지 않음 |
| `PermissionRequest` 출력 형식이 설계와 다름 | M3 시작 전에 공식 문서와 실제 동작으로 확인하고 이 문서를 고침 |
| Windows PTY와 경로 문제 | M0부터 CI에 Windows 포함. 경로 정규화 유틸을 한 곳에 둠 |
| 사용자 settings.json 손상 | 항상 백업, 병합, 확인 후 쓰기. 파싱 실패 시 쓰지 않음 |
| Anthropic이 비슷한 기능을 공식 제공 | 범위를 좁게 유지. 메모리 관리와 IDE 병행 사용에 집중 |

---

## 12. 열린 질문
- 첫 지시를 PTY 입력으로 넣을지, `claude` 실행 인자로 넘길지
- `Notification` 메시지만으로 `needs_input`을 안정적으로 판별할 수 있는지. 안 되면 보조 신호가 필요한지
- 기본 포트 번호 (임시 7311). 사용 중이면 자동으로 다음 포트를 쓸지
- observed 세션에 답장(텍스트 입력)할 방법이 있는지. 현재 설계는 "IDE 터미널에서 답하기" 안내만 함

---

## 13. 결정 기록
| 날짜 | 결정 | 이유 |
|---|---|---|
| 2026-11-10 | 백엔드 Rust, 프론트 React/TS | 단일 바이너리 배포, Windows PTY 지원, 나중에 Tauri로 같은 코어 재사용 |
| 2026-11-10 | 이름 pitwall, 이름에 "Claude" 넣지 않음 | 상표 문제 회피 |
| 2026-11-10 | 상태 판단은 hook 하나로만 | observed와 owned 세션을 같은 경로로 처리, 출력 파싱 의존 제거 |
| 2026-11-10 | 권한 타임아웃 시 결정을 내리지 않음 | pitwall이 대신 허용하는 일이 없도록 |
| 2026-11-10 | git은 명령 실행 방식 | worktree 지원, 사용자 git 설정 존중 |
