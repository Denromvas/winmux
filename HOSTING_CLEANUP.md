# Hosting Cleanup — denromvas.website

## TL;DR
Starting from **v0.1.2** WinMux no longer uses `denromvas.website` web hosting at all. The folder `/winmux/` on that hosting can be **deleted**.

## What moved where

| Resource | Old (web hosting) | New |
|---|---|---|
| `version.json` (updater) | `https://denromvas.website/winmux/version.json` | `https://raw.githubusercontent.com/Denromvas/winmux/main/web/version.json` |
| Installers (.exe / .zip) | `https://denromvas.website/winmux/winmux-*-v*.{exe,zip}` | `https://github.com/Denromvas/winmux/releases/download/v0.1.x/...` |
| Landing page (`url.landing` in palette) | `https://denromvas.website/winmux/` | `https://github.com/Denromvas/winmux` |
| Documentation (`url.docs` in palette) | `https://denromvas.website/winmux/docs/` | `https://github.com/Denromvas/winmux/blob/main/docs/TZ.md` |

## What stays self-hosted

The **telemetry endpoint** `telemetry.denromvas.website/v1/event` runs on the home server `your-home-server` (Pop!_OS) behind MikroTik NAT — it is NOT on ukraine.com.ua web hosting. No change needed.

## Safe to delete on web hosting

```
/winmux/index.html
/winmux/stats.html
/winmux/version.json
/winmux/docs/
```

Path on the host filesystem (mounted on dev machine):
```
/home/dromanyuk/sftp_mount/denromvas.website/www/winmux/
```

## When to delete

After every active v0.1.0/v0.1.1 install has updated to v0.1.2+. Until then keep `version.json` on the old URL pointing to the GitHub release so older installers can find the upgrade.

A safe sequence:
1. Release v0.1.2 (URLs already point to GitHub) — DONE
2. Wait ~2 weeks for install base to update
3. Check telemetry dashboard for v0.1.0/v0.1.1 still active → if zero, delete the folder
4. If non-zero, keep `version.json` only (≤1 KB) until they update or you stop caring
