# Code Audit (20260602-203452)

## Summary

`gstaldergeist` is a small (~750 LOC) Rust Telegram bot that assigns trash-disposal
duties among flatmates, scrapes two municipal waste schedules (Adliswil JSON API and
We-Recycle PDF), persists a schedule in SQLite, and sends reminders/emails. The code is
readable, modular, and uses modern, well-maintained crates with a committed `Cargo.lock`
and reproducible musl/scratch container build. CI runs `cargo check` on PRs.

However, the project is **not production-hardened**. The most serious issues are an
**unauthenticated command/callback surface** (any Telegram user can drive the bot,
including triggering a real e-mail to We-Recycle and exfiltrating the schedule), a
**guaranteed panic path** (`String::split_off(17)`) reachable from normal operation, and
**multiple `.unwrap()`/`.expect()` calls on network and lock operations** that will crash
the long-running scheduler task. Deployment also has secret-handling problems: a workflow
that base64-encodes a secret into build logs (bypassing GitHub's masking), and an `.env`
containing all secrets written world-readable (`0644`). There are **no tests at all**,
despite non-trivial regex/PDF parsing logic. None of these is a memory-safety issue
(Rust + safe code), but availability and authorization are weak.

## Findings

### Critical

**C1. No authorization check on any Telegram handler — anyone can drive the bot**
`src/answer_handler.rs:134` (`handle_callback_query`) and `src/answer_handler.rs:175`
(`handle_message`). The bot processes messages and callback queries from *any* chat/user.
`Config.flatmates` and `global_channel_id` exist but are never used to authorize the
sender. Consequences:
- Anyone who finds the bot can press the inline buttons. `new_bags` → `sure_bags`
  triggers `email::request_new_bags()` (`src/answer_handler.rs:113`), sending a real
  e-mail to the configured We-Recycle address from your account — an unauthenticated
  outbound action / spam vector.
- The `ping` command (`src/answer_handler.rs:182-209`) discloses internal state: current
  time, next scheduled trigger, and the **entire trash schedule** from the database.
- Pressing `done`/`cant` lets an outsider manipulate the shared task state machine
  (mark the chore done or "failed"), suppressing or forcing reminders for the real
  flatmates.

*Fix:* At the top of both handlers, resolve the sender's chat/user id and reject anything
not in `config.flatmates` (and restrict `ping`/admin output to known ids). Inline buttons
should also verify `query.from.id` against the intended recipient before mutating state or
sending mail. Call `bot.answer_callback_query` with a rejection for unauthorized users.

**C2. Guaranteed panic: `String::split_off(17)` on Telegram chat title**
`src/data_grabber.rs:119` and `src/data_grabber.rs:153`
(`chat_info.title.split_off(17)`). `split_off` panics if `17 > title.len()` **or** if byte
index 17 is not a UTF-8 char boundary (e.g. a title containing accented/emoji characters,
common in names). Any flatmate whose Telegram chat title is shorter than 17 bytes, or
whose 17th byte falls mid-codepoint, will panic `grab_today_food_master_name` /
`grab_tomorrow_food_master_name`. These run inside `collect_trashes_data` →
`send_scheduled_messages` (the spawned scheduler task), so the panic silently kills the
scheduler for the rest of the process lifetime while the dispatcher keeps running — the
bot appears alive but never sends reminders again.

*Fix:* Replace the magic-number `split_off(17)` with explicit, panic-free logic, e.g.
`title.char_indices().nth(17).map_or("", |(i, _)| &title[i..]).to_string()`, or better,
parse the name with a documented format. Add a test covering short and multibyte titles.

### High

**H1. `.unwrap()` on network calls in the scheduler path**
`src/data_grabber.rs:113` and `src/data_grabber.rs:147`
(`client.get(url).send().await.unwrap()`). If `api.telegram.org` is unreachable, slow, or
returns a transport error, this panics and (as in C2) kills the scheduler task. These two
functions also have no request timeout, so a hung connection blocks the loop.

*Fix:* Propagate errors with `?` (the functions are `async` and the call site already
returns `Result`), or handle the error and fall back to `"Error"` as the JSON branch
already does. Add a `reqwest::Client` timeout.

**H2. `.unwrap()` on Telegram sends in `weekly_update`**
`src/telegram_writer.rs:57-59` and `src/telegram_writer.rs:63-66`
(`bot.send_message(...).await.unwrap()`). Unlike `daily_update`/`send`, these two sends are
unwrapped. A transient Telegram API error (rate limit, network blip, user blocked the bot)
panics the scheduler task. This is the same availability failure mode as C2/H1 but in the
weekly path.

*Fix:* Use the same `match res { Ok/Err => log }` pattern used elsewhere in the file
(`src/telegram_writer.rs:9-14`).

**H3. Workflow exfiltrates a secret into build logs, bypassing masking**
`.github/workflows/hello-world.yml:10` — `run: echo ${{ secrets.ANSIBLE_USER }} | base64`.
GitHub masks raw secret values in logs, but base64-encoding the value defeats the mask: the
encoded string is printed verbatim and is trivially decodable by anyone who can read the
Actions logs. This file has no purpose other than leaking a secret and should be treated as
a finding regardless of intent.

*Fix:* Delete `hello-world.yml`. Never transform secrets before printing; never print
secrets at all.

**H4. `.env` with all secrets written world-readable (`0644`)**
`ansible/roles/deploy-gstaldergeist/tasks/main.yml:11-18`. The deployed `.env` contains
`EMAIL_PASSWORD`, `TELEGRAM_BOT_TOKEN`, etc., yet is created `mode: "0644"` (readable by
every local user on the host). Combined with H5 it may also contain unrelated secrets.

*Fix:* Set `mode: "0600"`. Consider Ansible Vault or podman secrets instead of a plaintext
file on disk.

### Medium

**M1. CI dumps *all* repository secrets into the app `.env`**
`.github/workflows/deploy.yml:62-74` and `.github/workflows/deploy-prod.yml:53-65`. The
"Generate Env File" step serializes `toJson(secrets)` and writes every secret (only
excluding the `ANSIBLE_` prefix) into `.env`. This pushes secrets the app never needs —
e.g. `GITHUB_TOKEN` — onto the production host's filesystem, widening the blast radius of a
host compromise.

*Fix:* Enumerate exactly the variables the app requires
(`TELEGRAM_*`, `EMAIL_*`, `ADDRESS`, `TO_EMAIL`) and write only those.

**M2. Adliswil grabber fetches only one month, silently dropping cross-month events**
`src/data_grabber/adliswil.rs:34-37`. The URL is built from `from.format("%m-%Y")` (the
start month only). On a Sunday the schedule window is `today + 7 days`
(`src/main.rs:137-140`); when that window crosses a month boundary, all events in the next
month are missed and never reported.

*Fix:* Query each month spanned by `[from, to]` and merge results.

**M3. `set_trashes` is not transactional**
`src/database.rs:54-72`. `DELETE FROM trashes` followed by row-by-row `INSERT` runs without
an explicit transaction. If any insert fails mid-loop, the table is left partially
populated (data already deleted). Concurrent reads from `get_all_trashes`
(`src/answer_handler.rs:196`, triggered by `ping`) can also observe a half-written table or
hit `SQLITE_BUSY`.

*Fix:* Wrap the delete+inserts in a single `conn.transaction()` and commit at the end;
reuse a prepared statement (already done) inside it.

**M4. Wasted Telegram API calls for unused fields**
`src/data_grabber.rs:180-181` populate `_master_name` (`grab_today_food_master_name`, a
network round-trip) and `_master_id` into struct fields prefixed `_` that are never read
anywhere. Every `collect_trashes_data` call (≥ daily, more on triggers) makes an extra
`getChat` request whose result is discarded.

*Fix:* Remove the unused `_master_name`/`_master_id` fields and the `grab_today_*` /
`today_food_master_id` functions, or actually use them.

**M5. Mutex poisoning turns one panic into permanent failure**
`src/main.rs:110,112,156,169,192`, `src/answer_handler.rs:23,58,192`,
`src/telegram_writer.rs:104,116` all use `lock().unwrap()`. If any thread panics while
holding `SharedTaskState` (very possible given C2/H1/H2), the mutex is poisoned and every
subsequent `lock().unwrap()` panics too, cascading the failure across both the dispatcher
and scheduler.

*Fix:* Remove the panic sources (C2/H1/H2); additionally consider
`lock().unwrap_or_else(|e| e.into_inner())` to recover from poisoning, or a non-poisoning
primitive.

### Low

**L1. `config()` returns `Result` but panics via `.expect()`**
`src/main.rs:31-52`. Every failure path uses `.expect()`, so the function can never return
`Err` and the `Result` signature is misleading. Acceptable as fail-fast startup behavior,
but it should either return real `Err(ConfigError(...))` or be typed to not return
`Result`.

**L2. `compute_next_trigger` keeps current seconds/nanos and may panic on DST**
`src/main.rs:116-129`. `with_hour(..).unwrap().with_minute(0).unwrap()` does not zero
seconds/subseconds, so triggers fire at e.g. `16:00:37`. The `.unwrap()`s can also panic on
the rare local DST transition where the target wall-clock time doesn't exist.

*Fix:* Build the target time with `and_hms_opt(h, 0, 0)` and handle the `LocalResult`
explicitly instead of unwrapping.

**L3. Fragile We-Recycle PDF parsing with magic constants**
`src/data_grabber/we_recycle.rs:31-65`. Region detection hinges on the literal substring
`"19"` (`regions.as_str().contains("19")`) and a hand-rolled regex; any layout/region
change silently yields zero dates with no alerting. The `\d+.pdf` href regex's `.` is
unescaped (matches any char) and may pick up relative URLs that then fail in
`client.get(pdf_url)`.

*Fix:* Escape the literal dot (`\d+\.pdf`), resolve relative URLs against the base, document
the `"19"` region assumption, and log when zero dates are extracted.

**L4. `cargo check` only — no build/test/clippy/fmt gate in CI**
`.github/workflows/merge-req.yml:21` runs `cargo check --no-default-features` only. It does
not compile tests, run `clippy`, or run `cargo test` (there are none — see L5). Note
`--no-default-features` may not reflect how the binary is actually built.

*Fix:* Add `cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt --check`.

**L5. No tests anywhere**
No `#[cfg(test)]` modules or `tests/` directory exist. The riskiest logic — date-range
filtering, the We-Recycle regex/PDF extraction (`extract_dates_from_txt`), the
`split_off(17)` name parsing, food-master rotation math, and `compute_next_trigger`
boundaries — is entirely unverified.

*Fix:* Add unit tests for `extract_dates_from_txt` (sample text fixtures),
`compute_next_trigger` (each hour branch + day rollover), the rotation modulo, and the
name-extraction helper (including short/multibyte inputs).

**L6. Duplicated grabber code**
`src/data_grabber.rs:96-123` vs `125-157` (`grab_tomorrow_food_master_name` vs
`grab_today_food_master_name`) are near-identical. Factor into one helper taking the target
date to reduce drift.

**L7. Bot token embedded in request URLs**
`src/data_grabber.rs:104-107,138-141`. The token is interpolated into the URL path. While
reqwest doesn't log URLs by default, URLs are the most likely thing to end up in proxy/error
logs. Prefer Telegram's documented behavior or at least avoid logging these URLs.

## Strengths

- **Memory-safe by construction**: idiomatic safe Rust, no `unsafe`, strong typing
  (`TrashType` enum with explicit `ToSql`/`FromSql`).
- **Modular layout**: data grabbers behind a `WasteGrabber` trait, separated
  email/telegram/database/error modules.
- **Parameterized SQL** throughout (`rusqlite::params!`, bound `?1/?2`) — no SQL injection.
- **Reproducible, minimal deployment**: committed `Cargo.lock`, multi-stage musl build into
  a `scratch` image, CA certs copied in, systemd unit with `Restart=always`.
- **Centralized error type** (`GstaldergeistError`) with `thiserror` and `From` conversions
  for clean `?` propagation in the parts that use it.
- **Current dependencies** on actively maintained crates (tokio, reqwest, teloxide,
  rusqlite, chrono).
- CI gates PRs with at least a compile check, and secrets are sourced from GitHub
  environments rather than committed.

## Recommendations

Prioritized next steps:

1. **Add authorization** to both Telegram handlers (C1) — reject senders not in
   `config.flatmates`; gate `ping` output and the bag-request action.
2. **Eliminate the panic paths** in the scheduler: fix `split_off(17)` (C2), replace
   network `.unwrap()`s with `?`/fallbacks and add timeouts (H1), and the `weekly_update`
   send `.unwrap()`s (H2). These three keep the long-running task alive.
3. **Fix secret handling in CI/deploy**: delete `hello-world.yml` (H3), set `.env` to
   `0600` (H4), and write only the env vars the app needs instead of all secrets (M1).
4. **Make `set_trashes` transactional** (M3) and fix the Adliswil cross-month gap (M2).
5. **Add a test suite and strengthen CI** (L4/L5): unit-test date parsing, trigger
   scheduling, rotation, and name extraction; add `cargo test`, `clippy -D warnings`, and
   `fmt --check`.
6. **Clean up**: remove unused master-name/id fields and their network calls (M4),
   deduplicate grabbers (L6), harden mutex usage against poisoning (M5), and address the
   low-severity parsing/config polish items (L1–L3, L7).

---

**Summary:** This is a tidy, idiomatic small Rust service with good build hygiene and no
memory-safety or injection issues, but it is not yet robust for unattended operation. The
dominant risks are authorization (any Telegram user can operate the bot, read the schedule,
and trigger outbound e-mail) and availability (a `split_off(17)` panic plus several
`.unwrap()`/`.expect()` calls on network, lock, and send operations will silently kill the
scheduler task while the process appears healthy), compounded by deployment-side secret
mishandling (a secret base64-dumped to CI logs and a world-readable `.env` full of
credentials) and a total absence of tests over non-trivial parsing logic. Addressing the
Critical and High findings — sender authorization, removing the panic paths, and fixing
secret exposure — would move this from a working hobby bot to a dependable one; the Medium
and Low items are maintainability and correctness polish that a small test suite and a
stricter CI gate would lock in.
