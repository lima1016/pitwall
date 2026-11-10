# CLAUDE.md

pitwall: Claude Code 세션 여러 개를 한 화면에서 관리하는 로컬 관제탑. 오픈소스(MIT).

## 먼저 읽을 것
- `docs/DESIGN.md`가 구현 기준이다. 작업 전에 해당 마일스톤 섹션을 읽는다.
- 설계와 다르게 구현해야 하면 코드보다 먼저 `docs/DESIGN.md`를 고치고 "결정 기록"에 한 줄 남긴다.
- [확인 필요] 표시가 있는 부분은 추측으로 구현하지 않는다. 공식 문서를 확인하거나 사용자에게 묻는다.

## 작업 방식
- 마일스톤 단위로 진행한다 (M0 → M1 → …). 현재 마일스톤의 완료 기준을 넘는 기능을 미리 만들지 않는다.
- 기능마다: 테스트 먼저 → 구현 → `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` 통과 → 커밋.
- 커밋은 작게, Conventional Commits 형식(`feat(core): …`, `fix(cli): …`).
- 마일스톤이 끝나면 완료 기준을 하나씩 확인한 결과를 요약해서 보고한다.

## 구조 규칙
- `crates/pitwall-core`: 도메인 로직. axum, HTTP 타입에 의존하지 않는다.
- `crates/pitwall-cli`: 실행 파일, 서버, CLI 명령.
- `web/`: React + TypeScript + Vite. 서버 API 타입은 한 파일(`web/src/api/types.ts`)에 모은다.
- 세션 상태 전환은 `pitwall-core/src/session/state.rs` 한 곳에서만 한다.
- 경로 처리는 `pitwall-core`의 경로 유틸을 거친다. Windows를 항상 고려한다.

## 절대 하지 말 것
- 사용자 실제 `~/.claude/settings.json`을 테스트나 개발 중에 직접 수정하지 않는다. 테스트는 임시 디렉터리를 쓴다.
- 서버를 `127.0.0.1` 외 주소에 바인딩하지 않는다.
- 권한 요청에 pitwall이 사용자 응답 없이 `allow`를 내보내는 코드를 만들지 않는다.
- `~/.claude/projects/**/*.jsonl` 같은 Claude Code 내부 파일 형식에 의존하지 않는다.
- 프롬프트 내용이나 도구 입력을 기본 로그에 남기지 않는다.

## 자주 쓰는 명령
```bash
cargo run -p pitwall-cli            # 서버 실행 (개발)
cargo test --workspace
cargo clippy --workspace -- -D warnings
cd web && npm run dev               # 프론트 개발 서버 (API는 프록시)
cd web && npm run build             # 내장용 빌드 → web/dist
```
