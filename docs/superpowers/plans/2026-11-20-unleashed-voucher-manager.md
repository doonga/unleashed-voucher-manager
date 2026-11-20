# Unleashed Voucher Manager Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the imported unifi-voucher-manager (UVM) code to Ruckus Unleashed guest passes, with the repository tooling, multi-arch image and release flow the spec describes, up to a `v1.0.0-rc.1` release candidate.

**Architecture:** The Axum backend's UniFi client (`backend/src/unifi_api.rs`) is replaced by an `unleashed` module (guest-list parsing, create requests, an authenticated session, pass-to-voucher mapping) behind an `UnleashedAPI` with the method set the handlers already call. The Next.js frontend changes only where Unleashed forces it. One image, built for `linux/amd64` and `linux/arm64` without emulation, published to GHCR by Forgejo Actions.

**Tech Stack:** Rust 2024 (axum 0.8, reqwest 0.13 with cookies and form, roxmltree, chrono-tz, tokio), Next.js 16 with TypeScript and Tailwind 4, Node's built-in test runner, mise, lefthook, oxfmt, Forgejo Actions, Docker Buildx with `cargo-zigbuild`.

**Spec:** `docs/superpowers/specs/2026-11-20-unleashed-voucher-manager-design.md`

## Global Constraints

- Controller calls go through `/admin/` pages: `GET login.jsp`, then `GET dashboard.jsp` (required before AJAX works), `POST _conf.jsp` (`text/xml`), `POST mon_createguest.jsp` (urlencoded form).
- CSRF token from the `HTTP_X_CSRF_TOKEN` login response header, falling back to `_csrfTokenVar.jsp`; sent as `X-CSRF-Token`.
- A `302` from an AJAX call means the session lapsed: one serialised re-login, one retry; a second `302` is an authentication error.
- Durations: minutes rounded up to whole hours, sent as `week`, `day` or `hour` (largest exact unit). Minutes are never sent.
- `count` 1 is `gentype=single`; 2 to 100 is `gentype=multiple`; anything else `400`. A custom key needs `count` 1.
- Names: whitespace replaced with `-`; ``' " < > & # ; \ ` | ! $ ( )`` rejected with `400`. Remarks: `<` and `>` rejected with `400`. Both printable ASCII only.
- Keys: 2 to 16 characters, no whitespace, `# & + " ' < >` or comma; uppercased.
- A create succeeds only if a new pass appears in the guest list afterwards; otherwise `502`. `KEY_DUPLICATED` is `409`; "Invalid Characters" is `400`; other controller failures `502`.
- Delete ids must be all ASCII digits, otherwise `400`.
- Rolling: name `[ROLLING]-<YYYYmmddHHMMSS>`, share limit 1, created only when no unused, unexpired rolling pass exists. Used means `used="true"` or `<client>` children; never `start-time` alone.
- An empty or zero `expire-time` is never expired.
- `GUEST_SUBNETWORK` uses the **last** `X-Forwarded-For` entry, trimmed, parsed as an IP (IPv4-mapped IPv6 normalised); anything unparsable is treated as outside the subnet.
- Environment: `UNLEASHED_URL`, `UNLEASHED_USERNAME`, `UNLEASHED_PASSWORD`, `UNLEASHED_SSID` (required), `UNLEASHED_HAS_VALID_CERT` (default `true`); `IS_LOGO_INVERTIBLE` defaults to `true`; every other UVM variable keeps its name and default.
- The password never appears in logs, error strings or `Debug` output.
- Image `ghcr.io/greyrock-labs/unleashed-voucher-manager`, `linux/amd64` and `linux/arm64`, no emulation on the runner: no `RUN` in stages built for the target platform.
- Committed files describe this repository only: no internal hostnames, addresses, SSIDs, credential store names or history. Examples use `192.0.2.x` and `example.com`. Scan every diff and commit message before pushing.
- Live controller rules: create only throwaway passes named `probe-<timestamp>` or created by the check, delete only those, never touch other passes, never send wrong credentials.
- Tool versions come from `.mise/config.toml`: Rust 1.100, Node 26.13 (matching the Dockerfile's base images).

## Review Focus

- **The arm64 image is built on amd64 and never run there.** Expected: both architectures start, pass the healthcheck and serve `/`. Pinned by Task 10, Step 4 (`docker run --platform` for each).
- **UVM's default names contain spaces** ("Quick Voucher"). Expected: created as `Quick-Voucher`, not a silent failure. Pinned by `creates_a_single_pass_with_a_spaced_name` (Task 6).
- **The controller answers OK but creates nothing.** Expected: `502`, never a phantom success. Pinned by `an_ok_that_creates_nothing_is_an_error` (Task 6).
- **A guest forges the first `X-Forwarded-For` entry** (`10.9.9.9, 192.168.50.7` from a guest at `192.168.50.7`). Expected: treated as `192.168.50.7` and blocked from admin pages. Pinned by `uses the last forwarded entry` (Task 8).
- **Staff type ordinary remarks** ("Bob's phone, room 4"). Expected: created. "<b>" gets a `400` naming the character. Pinned by `remarks_allow_apostrophes_and_commas` and `remarks_reject_angle_brackets` (Task 3).

---

### Task 1: Repository tooling

**Files:**
- Create: `.mise/config.toml`, `.mise/mise.lock`, `.lefthook.toml`, `.oxfmtrc.json`, `.editorconfig`, `.gitattributes`, `.markdownlint.yaml`, `.shellcheckrc`, `.renovaterc.json5`, `.superpowers/sdd/.gitignore` (contents `*` and `!.gitignore`)
- Modify: `.gitignore`, `.dockerignore`
- Delete: `backend/.cargo/config.toml` (its `target-cpu=native` makes images CPU-specific and breaks cross-compiling)
- Modify: `frontend/package.json` and `frontend/package-lock.json` (`name` becomes `unleashed-voucher-manager`)

**Interfaces:**
- Produces: mise tasks used by CI and later tasks: `build`, `test` (backend `cargo test --locked`, then frontend `npm test --if-present`), `fmt`, `fmt-check` (`cargo fmt --check`), `lint` (`cargo clippy --all-targets -- -D warnings`, `tsc --noEmit`), `oxfmt`, `oxfmt-check`, `frontend-build` (`npm ci && npm run build`), `dev`, `docker-build`, `docker-run`.

- [ ] **Step 1:** Write `.mise/config.toml` with tools `rust = "1.100"`, `node = "26.13"`, `lefthook`, `oxfmt` (latest releases, pinned by `mise lock`), the tasks above, and `postinstall = "lefthook install"`. Each task `dir`s into `backend/` or `frontend/` as needed.
- [ ] **Step 2:** `.oxfmtrc.json` sets `printWidth` 100 and ignores `backend/**`, `frontend/src/**`, `frontend/package-lock.json`. `.lefthook.toml`: pre-commit runs `fmt-check` and `oxfmt-check`; pre-push runs `test`.
- [ ] **Step 3:** `.renovaterc.json5` extends `config:recommended`, points the `github-actions` manager at `/^\.forgejo/workflows/.+\.ya?ml$/`, and sets `postUpdateOptions: ["npmDedupe"]` with `lockFileMaintenance` enabled, so `package-lock.json` is updated with `package.json`.
- [ ] **Step 4:** Run `mise install && mise run oxfmt && mise run fmt-check && mise run test && mise run frontend-build`.
  Expected: UVM's own backend tests pass, the frontend has no `test` script yet so `npm test --if-present` does nothing, and the frontend builds.
- [ ] **Step 5: Commit** `build: add mise, lefthook, oxfmt and Renovate configuration`

---

### Task 2: Guest-list parser (with fresh fixtures)

**Files:**
- Create: `backend/src/unleashed/mod.rs`, `backend/src/unleashed/guest.rs`
- Create: `backend/tests/fixtures/guest_list.xml`, `backend/tests/guest.rs`
- Modify: `backend/Cargo.toml` (`roxmltree`), `backend/src/lib.rs` (`pub mod unleashed;`)

**Interfaces:**
- Produces: `backend::unleashed::guest::GuestPass { id: String, name: String, key: String, remarks: String, share_number: u64, valid_time: u64, create_time: i64, start_time: Option<i64>, expire_time: Option<i64>, used: bool, client_macs: Vec<String> }` and `parse_guest_list(xml: &str) -> Result<Vec<GuestPass>, String>`. `name` is `full-name` falling back to `name`; `key` is `x-key` falling back to `key`; empty or `0` `start-time`/`expire-time` become `None`.

- [ ] **Step 1: Capture fixtures.** With the controller credentials cached owner-only in the scratchpad, run a throwaway probe (scratchpad, not committed) that logs in as the spec describes, creates `probe-<ts>` passes (one single with a custom key and remarks, one batch of 2, one duplicate key), saves the raw guest-list XML and the three raw `mon_createguest.jsp` bodies, then deletes exactly the passes it created and confirms the list matches the starting list. Sanitise keys, MACs, WLAN names and any non-probe pass names (`guest-1`, `Guest`, `00:00:00:00:00:01`) and save as `backend/tests/fixtures/guest_list.xml`, `create_single.txt`, `create_batch.txt`, `create_keydup.txt`. Keep response whitespace and the batch's script prefix exactly.
- [ ] **Step 2: Write failing tests** in `backend/tests/guest.rs`:
  - `parses_every_pass` – fixture yields the expected count; first pass's `id`, `name`, `key`, `share_number`, `valid_time`, `create_time`, `expire_time` equal the fixture's values.
  - `prefers_full_name_and_x_key`
  - `reads_client_macs_and_used`
  - `empty_times_are_none` – `start-time=""` and `expire-time=""` (and `"0"`) give `None`.
  - `missing_numbers_default_to_zero` – absent `share-number` is `0`.
  - `empty_list_is_empty` – `<ajax-response><response type="object"><guest-list/></response></ajax-response>` gives `[]`.
  - `garbage_is_an_error` – `"not xml"` gives `Err`.
- [ ] **Step 3:** `cd backend && cargo test --test guest` → compile error `unresolved import backend::unleashed`.
- [ ] **Step 4:** Implement with `roxmltree`; find every `guest` element anywhere in the document.
- [ ] **Step 5:** `cargo test --test guest` → all pass.
- [ ] **Step 6: Commit** `feat(backend): parse the Unleashed guest list`

---

### Task 3: Create requests and responses

**Files:**
- Create: `backend/src/unleashed/create.rs`, `backend/tests/create.rs`
- Modify: `backend/src/unleashed/mod.rs`
- Uses: the three `create_*.txt` fixtures from Task 2

**Interfaces:**
- Produces in `backend::unleashed::create`:
  - `pub const MAX_BATCH: u32 = 100;`
  - `pub struct CreateParams { pub count: u32, pub name: String, pub minutes: u64, pub share_limit: Option<u64>, pub key: Option<String>, pub remarks: Option<String>, pub ssid: String }`
  - `pub enum CreateOutcome { Accepted, KeyDuplicated(String), InvalidCharacters(String), Failed(String) }`
  - `duration_for_minutes(minutes: u64) -> Result<(u64, &'static str), String>` (`0` is an error)
  - `sanitize_name(name: &str) -> String`
  - `validate_name(name: &str) -> Result<String, String>` (sanitises, then checks characters)
  - `validate_remarks(remarks: &str) -> Result<(), String>`
  - `validate_key(key: &str) -> Result<String, String>` (uppercased)
  - `build_form(params: &CreateParams) -> Result<Vec<(&'static str, String)>, String>`
  - `parse_create_response(body: &str) -> CreateOutcome`
  - Error strings name the offending character, e.g. `remarks cannot contain '<'`.

- [ ] **Step 1: Write failing tests** in `backend/tests/create.rs`:
  - `rounds_minutes_up_to_hours` – `1 → (1,"hour")`, `61 → (2,"hour")`, `1440 → (1,"day")`, `10080 → (1,"week")`, `2880 → (2,"day")`, `0 → Err`.
  - `replaces_whitespace_in_names` – `"Quick Voucher" → "Quick-Voucher"`, tabs and runs of spaces become single `-` per whitespace char.
  - `rejects_name_characters_the_controller_refuses` – each of ``' " < > & # ; \ ` | ! $ ( )`` and `"é"` → `Err`.
  - `remarks_allow_apostrophes_and_commas` – `"Bob's phone, room 4"` → `Ok`.
  - `remarks_reject_angle_brackets` – `"<b>"` → `Err` containing `'<'`; `"café"` → `Err`.
  - `validates_keys` – `"ab"` → `"AB"`; `"a"`, 17 chars, `"a b"`, `"a#b"`, `"a,b"` → `Err`.
  - `single_form` – count 1, name `"Front Desk"`, 1440 min, limit 3, key `"abc123"`, remarks `"x"`, ssid `"Guest"` gives fields `gentype=single`, `fullname=Front-Desk`, `duration=1`, `duration-unit=day`, `limitnumber=3`, `key=ABC123`, `remarks=x`, `wlan=Guest`, `createToNum=1`. No limit sends `limitnumber=0`.
  - `batch_form` – count 3 gives `gentype=multiple`, `createToNum=3`; a key with count 3 → `Err`; count 0 and 101 → `Err`.
  - `parses_single_done`, `parses_batch_ok_with_prefix_lines`, `parses_key_duplicated` (message carried), `parses_invalid_characters`, `unreadable_response_fails` – against the fixtures and an inline `{"result":"Invalid Characters detected"}`-shaped body copied from the probe output.
  The exact form field names come from the captured request in Task 2's probe; the test asserts those names.
- [ ] **Step 2:** `cargo test --test create` → compile error.
- [ ] **Step 3:** Implement. `parse_create_response` strips everything before the first `{` and reads `result` (and the message field for errors).
- [ ] **Step 4:** `cargo test --test create` → all pass.
- [ ] **Step 5: Commit** `feat(backend): build and validate Unleashed guest pass creates`

---

### Task 4: Controller session and mock controller

**Files:**
- Create: `backend/src/unleashed/session.rs`, `backend/tests/support/mod.rs`, `backend/tests/session.rs`
- Modify: `backend/Cargo.toml` (reqwest features `cookies`, `form`, `json`, `query`), `backend/src/unleashed/mod.rs`

**Interfaces:**
- Produces in `backend::unleashed::session`: `pub enum SessionError { Connect(String), Auth(String) }` with `Display`; `pub struct Session` with `Session::new(url: &str, username: &str, password: &str, verify_tls: bool) -> Result<Session, SessionError>`, `async fn login(&self) -> Result<(), SessionError>`, `async fn conf(&self, xml: &str) -> Result<String, SessionError>`, `async fn form(&self, page: &str, fields: &[(&'static str, String)]) -> Result<String, SessionError>`. Login happens lazily on first request if `login` was not called.
- Test support `tests/support/mod.rs`: `Mock::start() -> Mock` (an in-process axum server), fields `url`, `state: Arc<Mutex<MockState>>`; methods `expire_sessions()`, `logins() -> usize`, `add_pass(name: &str, used: bool, expires_in: i64) -> u32`, `mark_used(id: u32)`, `pass_ids() -> Vec<u32>`, `passes() -> Vec<MockPass>`; `MockState` flags `always_redirect`, `token_in_script_only`, `ignore_creates`, `login_without_location`, `create_delay_ms: u64`, and `create_forms: Vec<Vec<(String,String)>>`; consts `USERNAME`, `PASSWORD`. The mock rejects AJAX from sessions that never loaded `dashboard.jsp`, answers `OK` without creating for names containing whitespace, names batches `Guest-N`, answers `KEY_DUPLICATED`, and serves `delobj` and `getconf`.

- [ ] **Step 1:** Change the reqwest features; `cargo build` succeeds.
- [ ] **Step 2:** Write `tests/support/mod.rs` as described.
- [ ] **Step 3: Write failing tests** in `backend/tests/session.rs`:
  - `logs_in_and_lists` – `conf(guest-list getconf)` returns XML containing `<guest-list`.
  - `logs_in_lazily_on_first_request`
  - `rejects_bad_credentials` – `SessionError::Auth`.
  - `reads_the_token_from_the_script_on_older_firmware` – `token_in_script_only`.
  - `logs_in_again_when_the_session_expires` – after `expire_sessions()`, the next call succeeds and `logins() == 2`.
  - `concurrent_requests_share_one_relogin` – 5 parallel calls after expiry, `logins() == 2`.
  - `gives_up_after_one_relogin` – `always_redirect` → `Auth`.
  - `unreachable_controller_is_a_connect_error` – port with nothing listening.
  - `login_errors_never_contain_the_password` – a login to an unreachable host and a `login_without_location` redirect both produce errors whose `to_string()` does not contain the password or `password=`.
- [ ] **Step 4:** `cargo test --test session` → compile error.
- [ ] **Step 5:** Implement as the spec's Session section says. One `tokio::sync::Mutex` guards the token; re-login is serialised under it. Log and format reqwest errors with `without_url()`; strip the query from any URL put in an error.
- [ ] **Step 6:** `cargo test --test session` → all pass.
- [ ] **Step 7: Commit** `feat(backend): add an Unleashed admin session with re-login`

---

### Task 5: Map guest passes to vouchers

**Files:**
- Create: `backend/src/unleashed/mapping.rs`, `backend/tests/mapping.rs`
- Modify: `backend/src/models.rs` (`Voucher`), `backend/src/unleashed/mod.rs`

**Interfaces:**
- Consumes: `GuestPass` (Task 2).
- Produces: `Voucher` without `data_usage_limit_mbytes`, `rx_rate_limit_kbps`, `tx_rate_limit_kbps`, with `#[serde(default)] pub remarks: String`. In `backend::unleashed::mapping`: `pub const ROLLING_PREFIX: &str = "[ROLLING]-";`, `format_time(unix: i64, tz: Tz) -> String` (UVM's existing date format), `is_used(&GuestPass) -> bool`, `is_expired(&GuestPass, now: i64) -> bool`, `to_voucher(&GuestPass, tz: Tz, now: i64) -> Voucher`, `is_rolling(&GuestPass) -> bool`, `rolling_name(created: DateTime<Tz>) -> String`, `current_rolling(passes: &[GuestPass], now: i64) -> Option<&GuestPass>`.

- [ ] **Step 1: Write failing tests** in `backend/tests/mapping.rs`:
  - `maps_every_field` – per the spec's mapping table, including `authorizedGuestCount` = client count and `timeLimitMinutes = valid_time / 60`.
  - `zero_share_limit_is_unlimited` – `share_number 0` → `authorized_guest_limit None`.
  - `activated_only_when_used` – `start_time` set but unused, no clients → `activated_at None`; `used` → `Some`.
  - `a_pass_without_expiry_never_expires` – `expire_time None` → `is_expired == false`, `expired == false`.
  - `rolling_name_format` – 2026-10-08 12:57:14 in `America/Toronto` → `"[ROLLING]-20261008125714"`.
  - `current_rolling_is_the_newest_unused_unexpired` – among rolling passes newer-used, older-unused, expired, and a non-rolling newer pass, returns the older unused rolling one.
  - `no_current_rolling_when_all_used_or_expired`.
- [ ] **Step 2:** `cargo test --test mapping` → compile error.
- [ ] **Step 3:** Change `Voucher` (UVM's `unifi_api.rs` still compiles because the new field defaults), implement the mapping.
- [ ] **Step 4:** `cargo test` → `mapping` passes, everything else still passes.
- [ ] **Step 5: Commit** `feat(backend): map guest passes to vouchers and rolling rules`

---

### Task 6: Switch the backend to Unleashed

**Files:**
- Create: `backend/src/unleashed_api.rs`, `backend/tests/api.rs`
- Delete: `backend/src/unifi_api.rs`; remove `percent-encoding` from `Cargo.toml`
- Modify: `backend/src/environment.rs`, `backend/src/models.rs`, `backend/src/handlers.rs`, `backend/src/main.rs`, `backend/src/lib.rs`

**Interfaces:**
- Consumes: Tasks 2 to 5.
- Produces: `backend::unleashed_api::{ApiConfig, UnleashedAPI, UNLEASHED_API}`. `ApiConfig { url, username, password, ssid: String, verify_tls: bool, timezone: Tz, rolling_voucher_duration_minutes: u64 }` with `ApiConfig::from_environment(&Environment) -> ApiConfig`. `UnleashedAPI::try_new(ApiConfig) -> Result<Self, SessionError>` and, all returning `Result<_, reqwest::StatusCode>`: `get_all_vouchers`, `get_vouchers(&VouchersGetRequest)`, `get_voucher_details(String)`, `get_newest_voucher`, `get_rolling_voucher -> Option<Voucher>`, `create_voucher(&VouchersCreateRequest) -> VouchersCreateResponse`, `create_rolling_voucher() -> Voucher`, `delete_vouchers_by_ids(Vec<String>) -> DeleteResponse`, `delete_expired_vouchers`, `delete_expired_rolling_vouchers`. `UnleashedAPI` holds `create_lock: tokio::sync::Mutex<()>`, held from the list before a create to the list after it, and across the whole rolling check-and-create. `VouchersCreateRequest` loses the data and rate fields and gains `code: Option<String>`, `remarks: Option<String>`. The rolling handler no longer reads `X-Forwarded-For`. Routes and JSON are otherwise UVM's.

- [ ] **Step 1: Write failing tests** in `backend/tests/api.rs` (against the Task 4 mock):
  - `startup_fails_on_bad_credentials` – `try_new` → `Err(SessionError::Auth(_))`.
  - `creates_a_single_pass_with_a_spaced_name` – `"Quick Voucher"` → one voucher named `Quick-Voucher`.
  - `duplicate_names_are_fine`
  - `creates_a_batch` – count 3 → 3 vouchers named `Guest-N`.
  - `custom_key_and_remarks` – code `check1`, remarks `"Bob's phone"` → code `CHECK1`, remarks preserved.
  - `duplicate_key_is_a_conflict` – `409`.
  - `invalid_requests_are_bad_requests` – count 0, count 101, key with count 2, name `"a&b"`, remarks `"<b>"` → `400`, and the mock saw no create.
  - `an_ok_that_creates_nothing_is_an_error` – `ignore_creates` → `502`.
  - `concurrent_creates_return_only_their_own_pass` – with `create_delay_ms`, two parallel creates each return exactly their own pass.
  - `lists_newest_first_and_filters_by_name`, `details_and_newest`.
  - `deletes_selected_passes`, `rejects_non_numeric_ids` (`"1'></guest><guest id='2"` → `400`, nothing deleted), `deleting_nothing_is_fine`.
  - `rolling_vouchers_are_for_one_device` – the create form for a rolling voucher has `limitnumber=1`.
  - `one_rolling_voucher_waits_at_a_time` – two sequential `create_rolling_voucher()` calls return the same id; after `mark_used(id)`, the next call creates a new one.
  - `concurrent_rolling_requests_create_one_voucher` – 5 parallel calls, one pass created, all return its id.
  - `expired_cleanup_skips_passes_without_expiry` – a pass with empty `expire-time` survives `delete_expired_vouchers`.
  - `survives_a_controller_reboot` – `expire_sessions()` between calls.
- [ ] **Step 2:** `cargo test --test api` → compile error.
- [ ] **Step 3:** Replace `environment.rs` (new variables, `UNLEASHED_HAS_VALID_CERT` default `true`), `models.rs` (drop UniFi-only types), add `unleashed_api.rs`, `git rm backend/src/unifi_api.rs`, update `lib.rs`, `handlers.rs`, `main.rs` (log `Connected to the Unleashed controller` on success).
- [ ] **Step 4:** `cargo test` → all binaries pass; `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` clean.
- [ ] **Step 5: Commit** `feat(backend)!: replace the UniFi client with Unleashed`

---

### Task 7: Startup, purge and redaction hardening

**Files:**
- Create: `backend/tests/startup.rs`, `backend/tests/tasks.rs`, `backend/tests/redaction.rs`
- Modify: `backend/src/main.rs`, `backend/src/lib.rs` (new `pub mod startup;` → `backend/src/startup.rs`), `backend/src/tasks.rs`, `backend/src/environment.rs`, `backend/src/unleashed_api.rs`

**Interfaces:**
- Produces: `backend::startup::retry_delay(attempt: u32) -> std::time::Duration` (5 s, doubling, capped at 300 s) and `backend::startup::describe(&SessionError) -> String` (names `UNLEASHED_USERNAME`/`UNLEASHED_PASSWORD` for `Auth`, `UNLEASHED_URL` for `Connect`, never the values); `backend::tasks::next_midnight(now: DateTime<Tz>) -> DateTime<Tz>`; hand-written `Debug` for `Environment` and `ApiConfig` printing `<redacted>` for the password.

- [ ] **Step 1: Write failing tests:**
  - `startup.rs`: `retry_delay_doubles_to_five_minutes` – attempts 0..=7 give 5, 10, 20, 40, 80, 160, 300, 300 seconds; `auth_errors_name_the_credentials` – `describe(&SessionError::Auth(..))` contains `UNLEASHED_PASSWORD` and not the password.
  - `tasks.rs`: `an_ordinary_night` (`America/Toronto`, 2026-11-20 15:00 → 2026-11-21 00:00), `just_after_midnight_waits_for_the_next_one`, `a_skipped_midnight_runs_at_the_first_valid_time` (`America/Santiago`, 2026-09-05 23:30 → 2026-09-06 01:00), `a_repeated_midnight_runs_at_the_first_one` (`America/Havana`, 2026-10-31 23:30 → the first 2026-11-01 00:00).
  - `redaction.rs`: `api_config_debug_hides_the_password`, `environment_debug_hides_the_password` – `format!("{:?}")` does not contain `"hunter2-do-not-print"` and contains `<redacted>`.
- [ ] **Step 2:** Run each → compile errors.
- [ ] **Step 3:** Implement. The startup loop in `main.rs` uses `retry_delay` and logs `describe`; the purge loop uses `next_midnight` and no longer `expect`s.
- [ ] **Step 4:** `cargo test` all pass; clippy and fmt clean.
- [ ] **Step 5: Commit** `fix(backend): back off at startup, survive DST midnights, redact the password`

---

### Task 8: Port the frontend

**Files:**
- Create: `frontend/src/utils/boolean.ts`, `frontend/src/utils/boolean.test.ts`, `frontend/src/utils/validation.ts`, `frontend/src/utils/validation.test.ts`, `frontend/src/utils/forwarded.ts`, `frontend/src/utils/forwarded.test.ts`
- Modify: `frontend/src/types/voucher.ts`, `frontend/src/utils/api.ts`, `frontend/src/components/tabs/CustomCreateTab.tsx`, `frontend/src/types/print.ts`, `frontend/src/types/config.ts`, `frontend/src/utils/format.ts`, `frontend/src/app/print/page.tsx`, `frontend/src/components/modals/VoucherModal.tsx`, `frontend/src/components/VoucherCard.tsx`, `frontend/src/components/tabs/TestTab.tsx`, `frontend/src/app/kiosk/page.tsx`, `frontend/src/app/welcome/page.tsx`, `frontend/src/proxy.ts`, `frontend/src/utils/runtimeConfig.ts` (or wherever `IS_LOGO_INVERTIBLE` is read), `frontend/src/app/layout.tsx`, `frontend/src/components/Header.tsx`, `frontend/tsconfig.json` (`allowImportingTsExtensions` if `node --test` needs `.ts` imports)

**Interfaces:**
- Consumes: backend JSON from Task 6 (`remarks`, no data or rate fields; create takes `code`, `remarks`; `409` key taken; `400` with the backend's message).
- Produces: `parseBoolean(value: unknown, fallback: boolean): boolean` (`true/yes/1/on`, `false/no/0/off`, case- and space-insensitive); `nameError(name: string): string | null`, `remarksError(remarks: string): string | null`, `keyError(key: string): string | null` with the same character rules as the backend; `clientIpFromForwarded(header: string | null): string | null` (last entry, trimmed, `::ffff:` stripped, `null` if not an IPv4/IPv6 address); constants `MAX_VOUCHER_COUNT = 100`, `MIN_VOUCHER_KEY_LENGTH = 2`, `MAX_VOUCHER_KEY_LENGTH = 16` in `utils/api.ts`; `PrintConfig` keeps `showName` and drops `showDataUsageLimit`, `showRxRateLimit`, `showTxRateLimit`.

- [ ] **Step 1: Write failing tests** (`node --test`), and add `"test": "node --test \"src/**/*.test.ts\""` to `frontend/package.json` (the quoted glob is required: the runner's Node treats a bare directory as a file):
  - `boolean.test.ts`: `environment strings become booleans` (`"true"`, `"TRUE"`, `" yes "`, `"1"` → true; `"false"`, `"0"`, `"no"` → false), `unreadable values use the fallback` (`undefined`, `""`, `"maybe"`), `real booleans pass through`.
  - `validation.test.ts`: `names reject what the controller refuses` (each character from the Global Constraints), `names allow dashes brackets and dots`, `remarks allow apostrophes and commas`, `remarks reject angle brackets`, `non-ASCII is rejected`, `keys follow the length and character rules`.
  - `forwarded.test.ts`: `uses the last forwarded entry` (`"10.9.9.9, 192.168.50.7"` → `"192.168.50.7"`), `single entry`, `strips IPv4-mapped prefix`, `rejects junk` (`"hello"`, `""`, `null` → `null`).
- [ ] **Step 2:** `cd frontend && npm test` → fails (modules missing).
- [ ] **Step 3:** Implement the three utilities; `npm test` → passes.
- [ ] **Step 4:** Change `types/voucher.ts` (drop data and rate fields, add `remarks: string`; create data gains optional `code`, `remarks`), then `npx tsc --noEmit` → errors list every remaining use. Fix them:
  - Custom Create: duration value plus unit select (hours, days, weeks) converted to `timeLimitMinutes`; no data or rate inputs; optional key and remarks inputs validated with `keyError`/`remarksError`; name validated with `nameError`; count above 1 disables name and key with a note that the controller names batch vouchers; a `409` shows "That key is already in use"; a `400` shows the backend's message.
  - Voucher card and modal: drop data and rate rows, show remarks when non-empty.
  - Print: keep UVM's current card and `showName`; drop the three rows and config keys.
  - TestTab: drop removed fields, add `remarks: ""`.
  - Welcome page: drop the comment about a `403` per-IP response.
  - Proxy: `GUEST_SUBNETWORK` uses `clientIpFromForwarded(request.headers.get("x-forwarded-for"))`; remove UVM's response-header `set("x-forwarded-for", …)` line.
  - Kiosk: re-fetch every 60 s without showing the spinner; when the fetch finds none, create one (as on load).
  - `IS_LOGO_INVERTIBLE` read through `parseBoolean(value, true)`; `DEFAULT_RUNTIME_CONFIG.IS_LOGO_INVERTIBLE = true`.
  - Rebrand: page titles, headings and metadata say "Unleashed Voucher Manager"; no "UniFi" text remains.
- [ ] **Step 5:** `npx tsc --noEmit && npm test && npm run build` → clean, build lists `/`, `/kiosk`, `/print`, `/welcome`. `grep -rn -i -e unifi -e dataUsage -e RateLimit frontend/src frontend/package.json` → no output.
- [ ] **Step 6: Commit** `feat(frontend)!: adapt voucher creation, display and gating to Unleashed` (one commit is fine; split utilities into their own commit if the diff reads better).

---

### Task 9: RUCKUS logos and dark-mode header

**Files:**
- Modify: `frontend/public/logo.svg`, `frontend/src/app/favicon.ico`, `frontend/src/app/favicon.svg`, `frontend/src/app/apple-touch-icon.png`, `frontend/src/components/Header.tsx`, `frontend/src/components/utils/WifiQr.tsx`
- Create: `frontend/public/qr-logo.svg`

**Interfaces:**
- Produces: `WifiQr`'s default `imageSrc` is `/qr-logo.svg`; the header shows `/logo.svg` and, when `IS_LOGO_INVERTIBLE` and the theme is dark, applies `invert hue-rotate-180`.

- [ ] **Step 1:** From `RUCKUS-Networks-Endorsed-Logo_black-orange.svg` in the RUCKUS logo suite (supplied locally by the maintainer; not committed), keep the dog, collar and bars, drop the wordmark and tagline, and centre them on a square white rounded tile with a black border inset so it is not clipped. Write it as `qr-logo.svg` and `logo.svg` (same artwork). Generate the favicons and touch icon from it.
- [ ] **Step 2:** Wire `WifiQr` to `/qr-logo.svg` and the header dark-mode filter. Theme follows the app's switcher (System, Light, Dark).
- [ ] **Step 3: Verify by eye:** render the SVG at 256 px; build and run the app without a controller (dummy `UNLEASHED_*` values) and screenshot the header in light and dark, and the QR modal in both. Decode the QR code in both themes (it must still scan). Show the screenshots to the maintainer.
- [ ] **Step 4:** `npm test && npm run build` pass.
- [ ] **Step 5: Commit** `feat(frontend): use the RUCKUS dog for the logo, QR code and icons`

---

### Task 10: Multi-arch container

**Files:**
- Modify: `Dockerfile`, `scripts/entrypoint.sh` and `scripts/healthcheck.sh` only if the new base requires it, `.dockerignore`
- Delete: `compose.yaml`
- Create: `docker-compose.yml`, `.env.example`

**Interfaces:**
- Consumes: tool versions from `.mise/config.toml` via build args `RUST_VERSION`, `NODE_VERSION`.
- Produces: `docker buildx build --platform linux/amd64,linux/arm64 .` builds without QEMU; the `runtime` stage is the final stage; the image listens on `3000` and keeps UVM's healthcheck.

- [ ] **Step 1:** Rewrite the `Dockerfile`:
  - Rust stage `FROM --platform=$BUILDPLATFORM rust:${RUST_VERSION}-alpine`; install `zig` and `cargo-zigbuild`; add both musl targets; map `TARGETARCH` (`amd64` → `x86_64-unknown-linux-musl`, `arm64` → `aarch64-unknown-linux-musl`); keep the dependency-caching step; copy the binary to a fixed path.
  - Node stages `FROM --platform=$BUILDPLATFORM node:${NODE_VERSION}-alpine`; install with `npm ci --os=linux --libc=musl --cpu=<target>` so any native module in the standalone output is the target's build.
  - Runtime `FROM node:${NODE_VERSION}-alpine` with only `COPY` (use `--chmod` and `--chown=node:node`), `USER node`, UVM's `ENTRYPOINT`/`HEALTHCHECK`. Drop `openssl` packages if reqwest builds with rustls.
- [ ] **Step 2:** `docker-compose.yml` and `.env.example` list every variable from the spec with defaults and `example.com` / `192.0.2.x` examples; image `ghcr.io/greyrock-labs/unleashed-voucher-manager:latest`; the logo mount examples for `logo.svg` and `qr-logo.svg`.
- [ ] **Step 3:** `docker buildx build --platform linux/amd64,linux/arm64 -t uvm:multi .` succeeds; `docker buildx build --platform linux/arm64 --load -t uvm:arm64 .` and the same for amd64.
- [ ] **Step 4:** For each architecture, `docker run --rm -d --platform linux/<arch> -p 3000:3000` with dummy `UNLEASHED_*` values; within 60 s the container is healthy, `curl -s localhost:3000/` returns the page, and the log shows the backend retrying the dummy controller with the backoff message. `find` the image's `node_modules` for `*.node` files and check with `file` that they match the architecture.
- [ ] **Step 5: Commit** `build: build amd64 and arm64 images without emulation`

---

### Task 11: Forgejo workflows

**Files:**
- Create: `.forgejo/workflows/test.yaml`, `.forgejo/workflows/lint.yaml`, `.forgejo/workflows/release.yaml`, `.forgejo/actionlint.yaml`

**Interfaces:**
- Consumes: mise tasks (Task 1), the Dockerfile (Task 10).
- Produces: CI on pull requests and `main`; images on `main` and `v*` tags. Secret: `GHCR_TOKEN` (GitHub username `doonga` in the login step, as the package owner's account).

- [ ] **Step 1:** `test.yaml` (pull requests and pushes to `main`): job `test` on `ubuntu-latest` with `jdx/mise-action`, running `fmt-check`, `lint`, `test`, `frontend-build`; job `image` on `docker` building `linux/amd64` without push, versions from `mise config get`.
- [ ] **Step 2:** `lint.yaml` (pull requests): actionlint via `bjw-s-labs/action-actionlint` with `.forgejo/actionlint.yaml` (self-hosted label `docker`, the Forgejo-specific ignores), and `mise run oxfmt-check`.
- [ ] **Step 3:** `release.yaml` (pushes to `main`, `v*` tags): read versions with mise, Buildx, GHCR login with `secrets.GHCR_TOKEN`, `docker/metadata-action` with `DOCKER_METADATA_ANNOTATIONS_LEVELS: manifest,index`, tags `type=semver,pattern={{version}}`, `type=semver,pattern={{major}}.{{minor}}`, `type=ref,event=branch`, `type=sha,prefix=sha-`; `org.opencontainers.image.source` and `url` labels and annotations pointing at `https://github.com/greyrock-labs/unleashed-voucher-manager`; build-push with `platforms: linux/amd64,linux/arm64`, `sbom: true`, `provenance: false`. Pin every action by commit SHA with the version in a comment. A pre-release tag must not produce `X.Y` or `latest` (metadata-action's semver patterns skip pre-releases for `{{major}}.{{minor}}` and `latest`; confirm in the action's output).
- [ ] **Step 4:** `mise exec -- actionlint` (or the container) on the three files → clean; `mise run oxfmt-check` → clean.
- [ ] **Step 5: Commit** `ci: test, lint and publish multi-arch images from Forgejo`

---

### Task 12: Documentation

**Files:**
- Modify: `README.md` (rewrite), `LICENSE` (add `Copyright (c) 2026 Todd Punderson` after UVM's line)
- Create: `AGENTS.md`

- [ ] **Step 1:** README, following UVM's structure, rewritten for Unleashed: what it is and that it is not affiliated with Ruckus or UVM's author; quick start with `docker-compose.yml`; every environment variable with default (cross-check against `environment.rs` and `entrypoint.sh`); creating the controller role (read-write admin) and user; the guest portal redirect to `/welcome`; rolling vouchers (one waiting voucher, single device, `/kiosk` creates the first); the reverse proxy and `X-Forwarded-For` requirement and the IPv6 note; logo replacement (`logo.svg`, `qr-logo.svg`, `IS_LOGO_INVERTIBLE`); troubleshooting (startup backoff, wrong credentials and the temporary login block, invalid characters).
- [ ] **Step 2:** `AGENTS.md` like a short operator's manual: purpose, layout, mise tasks, testing (mock controller, fixtures, `node --test` glob), the live check and its controller rules, the release process (`:main`, `vX.Y.Z-rc.N`, `vX.Y.Z` signed tags, every code change gets a release), Renovate lockfile note, and the documentation rules from the spec.
- [ ] **Step 3:** `grep -ho '"[A-Z_]\{6,\}"' backend/src/environment.rs | sort -u` and the `keys` list in `scripts/entrypoint.sh` → every name appears in the README and `.env.example`. `grep -rn -i -E 'unifi_|greyrock\.io|192\.168|10\.1\.' README.md AGENTS.md .env.example docker-compose.yml` → only the UVM attribution link and `UNIFI_*` names in a "differences from UVM" note, if any.
- [ ] **Step 4: Commit** `docs: document the Unleashed port, development and releases`

---

### Task 13: Publish, live check and release candidate

**Files:** none committed unless the check finds a bug (then fix test-first, commit, and repeat this task).

- [ ] **Step 1:** Scan the full diff since the spec commit and every commit message for internal details; push `main`. Watch `Test` and `Release` on Forgejo; both green. `Release` pushed `:main` and `:sha-<short>` for both architectures (`docker buildx imagetools inspect`).
- [ ] **Step 2:** Make the GHCR package public (`gh api -X PATCH` on the package visibility if the token allows; otherwise ask the maintainer to flip it) and confirm an anonymous `docker pull` works.
- [ ] **Step 3: Live check** with `ghcr.io/greyrock-labs/unleashed-voucher-manager:main` on this arm64 host against the controller (credentials from the scratchpad cache). Record existing pass ids first. Then: Quick Create twice (two `Quick-Voucher`); Custom Create single with 2 days, limit 3, key `CHECK1`, remarks `Bob's phone`; same key again (409 message); remarks `<b>` (400 message); batch of 3 (`Guest-N`); search; details (remarks, no data or speed rows); delete selected; print list and grid (headless screenshot with a hard timeout, since the page opens the print dialog and holds an SSE connection; otherwise ask the maintainer to look); `/kiosk` shows a `[ROLLING]-…` voucher with share limit 1; `POST /rust-api/vouchers/rolling` twice returns the same voucher; "Delete expired" reports without error; header light and dark by eye. Delete every pass the check created, leave any pre-existing rolling voucher alone, and confirm the guest list matches the starting ids.
- [ ] **Step 4:** Tag `v1.0.0-rc.1` (signed, annotated, notes in Added/Changed/Fixed style describing the app), push the tag, confirm `Release` publishes only `1.0.0-rc.1`.
- [ ] **Step 5:** Delete the cached credentials. Hand the maintainer the image reference and the deployment requirements from the spec. `v1.0.0` is tagged on the same commit after they confirm the deployment works.
