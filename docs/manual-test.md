# 수동 확인 체크리스트

마일스톤마다 자동 테스트로 확인하기 어려운 항목을 적는다.

## M0. 저장소 골격

- [ ] `cd web && npm ci && npm run build`
- [ ] `cargo run -p pitwall-cli` 실행 시 `pitwall: http://127.0.0.1:7311/` 출력
- [ ] 브라우저에서 `http://127.0.0.1:7311/` 접속 시 "pitwall" 제목이 있는 페이지가 보인다
- [ ] `http://127.0.0.1:7311/아무경로` 도 같은 페이지 (SPA fallback)
- [ ] 다른 PC나 `http://<내 LAN IP>:7311/` 로는 접속되지 않는다 (127.0.0.1 전용)
- [ ] `web/dist` 없이 빌드하면 "Web UI is not built yet" 안내 페이지가 보인다
- [ ] Ctrl+C로 서버가 정상 종료된다

## M1. hook 수신 + 읽기 전용 보드

실제 `~/.claude/settings.json`을 건드리지 않고 확인하는 방법:

```bash
export PITWALL_HOME=/tmp/pitwall-test            # 임시 데이터 폴더
pitwall init --settings /tmp/pitwall-test/claude-settings.json --yes
pitwall serve
claude --settings /tmp/pitwall-test/claude-settings.json   # 다른 터미널에서 (PITWALL_HOME도 같이)
```

- [ ] `pitwall init`을 두 번 실행하면 두 번째는 "No changes needed"
- [ ] `pitwall init --uninstall` 후 기존 hook만 남는다
- [ ] 서로 다른 폴더에서 띄운 claude 세션 3개가 보드에 나타난다
- [ ] 작업 중 → 권한 요청(또는 입력 대기) → 대기/종료가 실시간으로 바뀐다
- [ ] 권한 요청 카드가 강조되고 "나를 기다리는 세션"에 나타난다
- [ ] 등록하지 않은 폴더의 세션은 "미분류", 카드에서 바로 등록할 수 있다
- [ ] 한국어/English 전환이 되고 새로고침 후에도 유지된다
- [ ] 서버를 끈 상태(runtime.json이 남아 있어도)에서 claude가 평소처럼 동작한다
- [ ] 브라우저 주소창에서 `#token=`이 사라진다
- [ ] 토큰 없이 `/api/sessions` 호출 시 401
