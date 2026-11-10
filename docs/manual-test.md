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
