# Code Signing Setup (SignPath.io — free for OSS)

The CI workflow `.github/workflows/sign.yml` is already in the repo. It fires on
every published release and replaces the unsigned `.exe` assets with signed ones.
It stays dormant until you complete the one-time setup below.

## Why this matters

Right now every user who runs the installer gets a **"Windows protected your PC"**
SmartScreen warning, because the `.exe` is unsigned. Code signing removes that —
the single biggest barrier to people actually installing WinMux.

SignPath.io offers **free certificates and signing for open-source projects**.

## One-time setup (~20 min, done once)

### 1. Make sure the repo qualifies
- Public ✅ (already done)
- OSI license ✅ (MIT)
- Has releases ✅

### 2. Register at SignPath
1. Go to https://about.signpath.io/product/open-source and apply for the free OSS plan.
2. Connect your GitHub account, select the **Denromvas/winmux** repository.
3. SignPath creates an **Organization** and a **Project** for you.

### 3. Create the signing policy + artifact config
In the SignPath dashboard:
- **Signing policy** slug: `release-signing`
- **Artifact configuration** slug: `installers` — set it to accept `*.exe`

(These two slugs are referenced in `sign.yml`. If you name them differently,
update the `signing-policy-slug` and `artifact-configuration-slug` lines.)

### 4. Add 3 GitHub secrets
Repo → **Settings → Secrets and variables → Actions → New repository secret**:

| Secret | Where to find it |
|---|---|
| `SIGNPATH_API_TOKEN` | SignPath → User settings → API tokens |
| `SIGNPATH_ORG_ID` | SignPath → Organization → ID (GUID) |
| `SIGNPATH_PROJECT_SLUG` | The project slug, e.g. `winmux` |

### 5. Test it
- Publish any release (or re-run the workflow manually: Actions → "Sign release artifacts" → Run workflow → enter a tag).
- The workflow downloads the release `.exe` files, submits them to SignPath,
  waits for signing, and re-uploads the signed versions (`--clobber`).
- Download the installer again — right-click → Properties → **Digital Signatures**
  tab should now show a valid signature, and SmartScreen no longer warns.

## Notes
- SignPath OSS signing may require **manual approval per release** on the free tier
  (you click "approve" in their dashboard). That's fine for our release cadence.
- The signing happens on GitHub-hosted runners; no secrets ever touch a local machine.
- If you ever want EV-level (instant SmartScreen reputation), that's a paid cert —
  not needed for OSS, reputation builds over downloads.
