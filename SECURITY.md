# Security Policy

## Scope

This policy covers **EasyZapret only**: the desktop app in this repository (UI, updater, installer, and the code that downloads and launches third-party tools).

It does **not** cover:

- [FlowSeal/zapret-discord-youtube](https://github.com/Flowseal/zapret-discord-youtube)
- [FlowSeal/tg-ws-proxy](https://github.com/Flowseal/tg-ws-proxy)
- [bol-van/zapret](https://github.com/bol-van/zapret) / WinDivert
- Cloudflare WARP / the 1.1.1.1 client
- antivirus false positives around `winws.exe` or `WinDivert64.sys`

Report issues in those projects to their maintainers.

EasyZapret does not vendor zapret or tg-ws-proxy in this repository. On first run it downloads official GitHub releases and stores them under `C:\EasyZapret`.

## Supported versions

Only the **latest published release** on [Releases](https://github.com/laciodanya/easyzapret/releases/latest) gets security fixes.

| Version | Supported |
| ------- | --------- |
| Latest 0.5.x release | Yes |
| Older 0.5.x builds | No — update to the latest installer |
| < 0.5.0 | No |

Older builds may not verify updater signatures against the current key. If in-app update fails, install the latest `.exe` from Releases.

There is no LTS. Pre-release / unsigned local builds are unsupported.

## What to report

Please report privately if you find a problem in EasyZapret that could:

- run unexpected code or installers
- download components from a host that is not the official GitHub release
- bypass updater signature checks
- escalate privileges beyond what the user already granted (the app is intended to run as Administrator)
- leak local data (lists, logs, Telegram proxy secret) off the machine
- tamper with files outside `C:\EasyZapret` and the app install directory without the user asking

Do **not** use this channel for:

- “Discord/YouTube still blocked” / strategy not working
- antivirus quarantining WinDivert
- feature requests
- issues in zapret, tg-ws-proxy, or WARP itself

## How to report

Use GitHub’s private reporting:

**Security → Report a vulnerability** on this repository.

If that form is unavailable, open a **private** GitHub Security advisory, or contact the maintainer via the GitHub profile. Do not post exploit details, payloads, or proof-of-concept in public Issues or Discussions.

Include:

- EasyZapret version (Settings → About)
- Windows version
- whether the app was running as Administrator
- steps to reproduce
- what you expected vs what happened

Do not attach full `C:\EasyZapret` dumps. Redact logs if they contain host lists or proxy secrets.

## What happens next

This is a one-person project. There is no SLA.

- You should get an acknowledgement when the report is seen (often within a few days, sometimes longer).
- If it is in scope, a fix will be aimed at the next release when possible.
- If it is out of scope or not a vulnerability, you will be told and pointed to Issues/Discussions if it is a normal bug.
- Please do not disclose the issue publicly until a release is out, or until you are told it will not be fixed.

Credit in the release notes is optional and only if you want it.

## Hardening notes (not a guarantee)

- Install only from [github.com/laciodanya/easyzapret/releases](https://github.com/laciodanya/easyzapret/releases).
- The NSIS installer is not signed with a commercial Authenticode certificate, so SmartScreen may warn. That is expected; it is not a vulnerability by itself.
- In-app updates are signed with Tauri’s updater key. A signature mismatch means the build does not match the key baked into your installed app — do not ignore that; install the latest official installer instead.
- EasyZapret requires Administrator rights because zapret/WinDivert need them. That is a design requirement, not a bug.
- Use at your own risk and follow the laws of your country. This project is a UI around existing tools, not a security product and not an exploit kit.
