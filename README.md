# Unleashed Voucher Manager

A touch-friendly web application for managing guest passes on Ruckus Unleashed
controllers: create them, print them, show the current one on a kiosk, and
roll a new one automatically each time a guest connects.

It is a port of
[unifi-voucher-manager](https://github.com/etiennecollin/unifi-voucher-manager)
by Etienne Collin to Ruckus Unleashed, and keeps its interface and features
wherever the controller allows.

<!-- vim-markdown-toc GFM -->

- [Features](#features)
- [Quick start](#quick-start)
  - [Images](#images)
  - [Docker Compose](#docker-compose)
  - [Without Docker](#without-docker)
- [Configuration](#configuration)
  - [Controller account](#controller-account)
  - [Rolling vouchers and the kiosk page](#rolling-vouchers-and-the-kiosk-page)
  - [Reverse proxy](#reverse-proxy)
  - [Logos](#logos)
  - [Environment variables](#environment-variables)
- [Troubleshooting](#troubleshooting)
- [Credits](#credits)

<!-- vim-markdown-toc -->

## Features

- **Quick Create**: a voucher with a preset duration, from 1 hour to 30 days.
- **Custom Create**: choose the number of vouchers (up to 100 at once), name,
  duration in hours, days or weeks, guest limit, a custom key and remarks.
  Unleashed has no per-voucher data or speed limits; those are set per WLAN
  or role on the controller.
- **Browse vouchers**: search by name, view details, select and delete in
  bulk, delete expired vouchers.
- **Print vouchers**: list or grid layout, friendly to thermal printers, with
  the voucher name, code, duration, guest limit and Wi-Fi details.
- **WiFi QR code**: lets guests join the network by scanning.
- **Rolling vouchers**: single-device vouchers handed out one at a time; the
  next one is created as soon as the current one is used.
- **Kiosk page** (`/kiosk`): the current rolling voucher and the WiFi QR code,
  updated live.
- **Interface**: touch-friendly, dark and light mode, notifications,
  replaceable logos.
- **Architecture**: a Next.js frontend and an Axum (Rust) backend in one
  container, published for `linux/amd64` and `linux/arm64`. Only the backend
  talks to the controller, so the controller credentials never reach the
  browser.

## Quick start

### Docker Compose

1. Download `docker-compose.yml` and `.env.example` from this repository.
2. Copy `.env.example` to `.env` and fill it in (see
   [Environment variables](#environment-variables)).
3. Start it:

   ```bash
   docker compose up -d
   ```

4. Open `http://localhost:3000`.

The image is `ghcr.io/greyrock-labs/unleashed-voucher-manager`. It runs as
the `node` user (uid 1000) and listens on port 3000.

### Images

Images are published for `linux/amd64` and `linux/arm64`, tagged with the
exact version (for example `1.0.0`) and `latest`. Version tags never change
once published, so pin one, ideally with its digest, and let Renovate propose
updates. Version numbers follow semver: a patch release only fixes things or
updates dependencies, a minor release adds features, and a major release may
need configuration changes. Release notes are on the GitHub releases page.

Images are signed with cosign. To verify one:

```bash
cosign verify \
  --key https://raw.githubusercontent.com/greyrock-labs/unleashed-voucher-manager/main/cosign.pub \
  ghcr.io/greyrock-labs/unleashed-voucher-manager:1.0.0
```

### Without Docker

1. Install [mise](https://mise.jdx.dev), clone this repository and run
   `mise install` for the pinned Rust and Node versions.
2. Set the environment variables in your shell, or in a `.env` file in
   `backend/` and use the backend's `dotenv` feature.
3. Start the backend and frontend:

   ```bash
   # Backend (without a .env file)
   cd backend && cargo run --release

   # Backend (with a .env file)
   cd backend && cargo run --release --features dotenv

   # Frontend (development)
   cd frontend && npm install && npm run dev

   # Frontend (release)
   cd frontend && npm ci && npm run build && npm run start
   ```

4. Open `http://localhost:3000`.

## Configuration

### Controller account

The app logs in to the controller's admin interface with a username and
password. The account needs a role with **read-write admin privilege**.
Unleashed's built-in Guest Pass Manager role can list and create guest passes
but cannot delete them, so delete, cleanup and rolling vouchers would not
work with it.

In the Unleashed web interface, create a role with read-write admin
privilege, then a user in that role, and give that user's credentials to the
app.

### Rolling vouchers and the kiosk page

Rolling vouchers give each guest their own code: each one works on a single
device, and as soon as a guest uses the current one, the app creates the
next and the kiosk page shows it.

> [!IMPORTANT]
> Rolling vouchers need the guest WLAN to send guests to the app after they
> log in. In the Unleashed web interface, open the guest WLAN's guest access
> (captive portal) settings and set **Redirect user to this website** to the
> app's `/welcome` page, for example `https://voucher.example.com/welcome`.
>
> Without this, vouchers do not roll when guests connect.

> [!CAUTION]
> Set `GUEST_SUBNETWORK` to the guest network's subnet. Guests on it can then
> reach only `/welcome`, not the voucher management pages. Without it, guests
> can create and delete vouchers themselves.

How it works:

1. Opening `/kiosk` shows the waiting rolling voucher, creating one if none
   exists. The app never creates one by itself, so nothing appears until a
   kiosk is opened.
2. A guest logs in with it and the captive portal sends them to `/welcome`.
3. `/welcome` asks for the next rolling voucher. One is created only when no
   unused one is waiting, so reloading the page creates nothing more, and two
   guests arriving together get one new voucher between them.
4. Rolling vouchers are named `[ROLLING]-<timestamp>` and allow one device.
5. The kiosk re-checks every minute. If the waiting voucher was used without
   `/welcome` loading, or expired unused (when the guest portal counts time
   from issue), it creates the next one.
6. At midnight (in `TIMEZONE`) expired rolling vouchers are deleted, or all
   expired vouchers if `PURGE_ALL_EXPIRED_VOUCHERS` is set. Unleashed usually
   removes expired passes itself, so this rarely finds anything.

After logging in, guests must still be able to reach the app for the redirect
to `/welcome` to load. If the guest WLAN's restricted subnet rules block the
app's address, allow it there; a walled garden entry only applies before
login.

### Reverse proxy

`GUEST_SUBNETWORK` decides by the client's address, which the app takes from
the **last** entry of `X-Forwarded-For`. The reverse proxy directly in front
of the app must append (or set) the address it received the connection from;
earlier entries come from the client and can be forged. Do not expose the
app's port to the guest network directly.

`GUEST_SUBNETWORK` takes one IPv4 subnet. If the app's hostname also has an
IPv6 address, guests connecting over IPv6 are not covered.

### Logos

The header shows `logo.svg` and the centre of the Wi-Fi QR code shows
`qr-logo.svg`. Both default to the RUCKUS dog on a white tile. In dark mode
the header logo is inverted with its colours rotated back, so the dog turns
white on a black tile and keeps its orange; the QR logo is never inverted.

- With Docker, mount an SVG at `/app/frontend/public/logo.svg` or
  `/app/frontend/public/qr-logo.svg`. There are examples in
  `docker-compose.yml`. The mount destinations, including the file names,
  **cannot be changed**.
- Without Docker, replace the files in `./frontend/public/`.
- Set `IS_LOGO_INVERTIBLE=false` for a header logo that should not be
  inverted in dark mode.

### Environment variables

Required variables come first.

- **`UNLEASHED_URL`: `string`** (_Required_)
  - URL of the Unleashed controller, with protocol.
  - Example: `https://unleashed.example.com`
- **`UNLEASHED_USERNAME`: `string`** (_Required_)
  - A controller user whose role has read-write admin privilege. See
    [Controller account](#controller-account).
- **`UNLEASHED_PASSWORD`: `string`** (_Required_)
  - That user's password.
- **`UNLEASHED_SSID`: `string`** (_Required_)
  - The guest WLAN that vouchers are created on, as named on the controller.
  - Example: `Guest WiFi`
- **`UNLEASHED_HAS_VALID_CERT`: `bool`** (_Optional_)
  - Whether the controller's certificate is trusted. Set to `false` if it
    serves its default self-signed certificate.
  - Default: `true`
- **`GUEST_SUBNETWORK`: `IPv4 CIDR`** (_Optional_)
  - Guests on this subnet can reach only `/welcome`. See
    [Rolling vouchers and the kiosk page](#rolling-vouchers-and-the-kiosk-page).
  - Example: `192.0.2.0/24`
- **`TIMEZONE`: [timezone identifier](https://en.wikipedia.org/wiki/List_of_tz_database_time_zones#List)**
  (_Optional_)
  - Used to show dates and to schedule the midnight cleanup.
  - Default: `UTC`
- **`ROLLING_VOUCHER_DURATION_MINUTES`: `minutes`** (_Optional_)
  - How long a rolling voucher grants access. Rounded up to whole hours,
    because the controller only accepts hours, days and weeks.
  - Default: `480`
- **`PURGE_ALL_EXPIRED_VOUCHERS`: `bool`** (_Optional_)
  - When `true`, the midnight cleanup deletes all expired vouchers; when
    `false`, only expired rolling vouchers.
  - Default: `false`
- **`WIFI_SSID`: `string`** (_Optional_)
  - SSID for the QR code. The QR code needs both `WIFI_SSID` and
    `WIFI_PASSWORD`.
- **`WIFI_PASSWORD`: `string`** (_Optional_)
  - Password for the QR code. Set to `""` for an open network.
- **`WIFI_TYPE`: `WPA|WEP|nopass`** (_Optional_)
  - Security type for the QR code. Defaults to `WPA` with a password and
    `nopass` without.
- **`WIFI_HIDDEN`: `bool`** (_Optional_)
  - Whether the SSID is hidden.
  - Default: `false`
- **`IS_LOGO_INVERTIBLE`: `bool`** (_Optional_)
  - Whether the header logo is inverted in dark mode. `true`, `yes`, `1` and
    `on` enable it; `false`, `no`, `0` and `off` disable it.
  - Default: `true`
- **`PRINT_CONFIG`: `JSON object`** (_Optional_)
  - Which fields printed vouchers show. Omitted fields default to `true`.
  - Default:
    `{"showName":true,"showLogo":true,"showDuration":true,"showMaxGuests":true,"showId":true,"showPrintTime":true}`
- **`FRONTEND_BIND_HOST`: `IPv4`** (_Optional_), default `0.0.0.0`
- **`FRONTEND_BIND_PORT`: `u16`** (_Optional_), default `3000`
- **`FRONTEND_TO_BACKEND_URL`: `URL`** (_Optional_), default
  `http://127.0.0.1`
- **`BACKEND_BIND_HOST`: `IPv4`** (_Optional_), default `127.0.0.1`
- **`BACKEND_BIND_PORT`: `u16`** (_Optional_), default `8080`
- **`BACKEND_LOG_LEVEL`: `trace|debug|info|warn|error`** (_Optional_),
  default `info`

## Troubleshooting

- **The backend keeps retrying the connection at startup**
  - It waits 5 seconds, then twice as long each time, up to 5 minutes, and
    the log says whether the controller refused the login or could not be
    reached.
  - Check `UNLEASHED_URL` is correct and reachable from the container.
  - Check `UNLEASHED_HAS_VALID_CERT` matches the controller's certificate.
  - Check the username and password, and that the role has read-write admin
    privilege. After a few failed logins the controller refuses logins from
    that address for a short while, even with the right password; fix the
    setting and the app reconnects once that passes.
- **Creating a voucher fails**
  - Check `UNLEASHED_SSID` names an existing guest WLAN.
  - Names cannot contain ``' " < > & # ; \ ` | ! $ ( )``, remarks cannot
    contain `<` or `>`, and both must be plain ASCII. Spaces in names become
    dashes.
  - A custom key must be 2 to 16 characters, without spaces or
    `# & + " ' < > ,`, and not already in use.
- **Vouchers do not roll when guests connect**
  - Check the guest WLAN redirects to the app's `/welcome` page, and that
    logged-in guests can reach the app (see
    [Rolling vouchers and the kiosk page](#rolling-vouchers-and-the-kiosk-page)).
  - Check a kiosk has been opened at least once, which creates the first
    rolling voucher.
- **The WiFi QR code button is disabled**
  - Set `WIFI_SSID` and `WIFI_PASSWORD`, and check the browser console for
    configuration errors.

For more detail, run with `BACKEND_LOG_LEVEL="debug"` and check
`docker logs unleashed-voucher-manager`.

## Credits

Based on [unifi-voucher-manager](https://github.com/etiennecollin/unifi-voucher-manager)
by Etienne Collin, under the MIT License. The Unleashed protocol details were
worked out with help from [FetchPass](https://github.com/fmuffat/FetchPass)
and [aioruckus](https://github.com/ms264556/aioruckus).

This is an independent, unofficial project. It is not affiliated with,
endorsed by, or sponsored by Ruckus Networks or Ubiquiti Inc. Ruckus,
Unleashed, UniFi and all associated trademarks are the property of their
respective owners.
