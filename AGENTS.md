# AGENTS.md

## Purpose

`unleashed-voucher-manager` is a web app for Ruckus Unleashed guest passes:
create, browse, print and delete them, and hand out single-device rolling
vouchers on a kiosk page. It is a port of
[unifi-voucher-manager](https://github.com/etiennecollin/unifi-voucher-manager)
(UVM), maintained on its own. The design and the verified controller
behaviour are in `docs/superpowers/specs/2026-11-20-unleashed-voucher-manager-design.md`;
read it before changing how the app talks to the controller.

Image: `ghcr.io/greyrock-labs/unleashed-voucher-manager` (`linux/amd64`,
`linux/arm64`). GitHub mirror: `greyrock-labs/unleashed-voucher-manager`.

## Layout

```text
backend/                     Axum (Rust); the only part that talks to the controller
  src/unleashed/guest.rs     parse the guest list
  src/unleashed/create.rs    build create requests; name, key and remarks rules; read responses
  src/unleashed/session.rs   login, dashboard step, CSRF token, one shared re-login on 302
  src/unleashed/mapping.rs   guest pass -> Voucher JSON; rolling voucher rules
  src/unleashed_api.rs       operations the handlers call; the create lock
  src/startup.rs             connection backoff and messages
  src/tasks.rs               midnight purge
  tests/                     one file per module; support/mod.rs is the mock controller
  tests/fixtures/            real controller responses, sanitised
frontend/                    Next.js; src/proxy.ts forwards /rust-api/* and applies GUEST_SUBNETWORK
scripts/                     entrypoint, process wrapper and healthcheck for the container
Dockerfile                   amd64 and arm64 without emulation (cargo-zigbuild, sharp swap)
```

## Tooling

mise pins Rust, Node, lefthook and oxfmt and holds the tasks (`mise tasks`):
`mise run test`, `lint`, `fmt-check`, `oxfmt-check`, `frontend-build`,
`docker-build`. lefthook runs the format checks before commits and the tests
before pushes. oxfmt formats YAML, JSON and Markdown only; `backend/` and
`frontend/src/` keep UVM's formatting so later UVM changes port cleanly.

## Testing

- Write the failing test first and watch it fail.
- The mock controller reproduces observed behaviour: the dashboard step,
  CSRF checks, session expiry, names with whitespace that create nothing,
  `KEY_DUPLICATED`, invalid characters, `Guest-N` batch names. Extend it when
  the real controller turns out to behave differently.
- Keep fixtures sanitised: no real keys, MACs, WLAN names or pass names.
- Frontend tests are `src/**/*.test.ts`, run by `node --test`; keep the
  quoted glob in `npm test`.
- CI never talks to a controller.

The frontend runs without a controller, which is enough to check the UI:
start the image with dummy `UNLEASHED_*` values. The backend only listens
once it reaches a controller, so the container is not healthy then.

## Live testing against a controller

The controller serves real guests, so:

- **Never send wrong credentials.** A few failed logins make the controller
  refuse logins from that address for a while, even with the right password,
  with no event or alarm. Unit tests cover that path.
- Ask before writing. Keep any copy of the credentials outside the
  repository with owner-only permissions and delete it afterwards.
- Record the existing passes first, create only passes with test-like names
  (`probe-<time>`), delete only the ids you created, and confirm the list
  matches afterwards. Check names via `full-name`: some firmware lists no
  `name` attribute.
- A rolling voucher waiting on a kiosk belongs to the deployment: do not use
  it up or delete it. With one waiting, a rolling request returns it without
  creating anything, a safe check of the rolling path.
- Raw probing: `GET /admin/login.jsp?username=...&password=...&ok=Log In`,
  take the `HTTP_X_CSRF_TOKEN` header, load `dashboard.jsp`, then post to
  `/admin/_conf.jsp` (`text/xml`) or `/admin/mon_createguest.jsp` (form) with
  `X-CSRF-Token`.

## Workflow and releases

- Commit to `main` as focused, signed, Conventional Commits; push low-risk
  changes without asking, and ask before anything that publishes or could
  break a deployment.
- Pushes to `main` publish `:main` and `:sha-<short>`. Every change to code
  or the image gets a release: wait for the Release workflow on `main` to
  pass (it runs the tests before publishing `:main`), then push a signed
  annotated tag. `vX.Y.Z` publishes `X.Y.Z`, `X.Y` and `latest`;
  `vX.Y.Z-rc.N` publishes only `X.Y.Z-rc.N`. Tag notes list Added, Changed
  and Fixed for people running the app.
- The release workflow needs the repository secret `GHCR_TOKEN`, a GitHub
  classic PAT with `write:packages`.
- Renovate updates dependencies; an npm update must change
  `package-lock.json` in the same commit, or `npm ci` fails in CI.

## Documentation rules

- Each change updates the README, the spec or this file where it affects
  them, ideally in the same commit.
- Describe this repository only: no internal hostnames, addresses, SSIDs,
  usernames or credential store names, and no history of earlier attempts.
  Use `example.com` and `192.0.2.x` in examples.
- Grep every diff and commit message for internal details before pushing;
  amending afterwards does not remove a pushed commit from the remotes.
