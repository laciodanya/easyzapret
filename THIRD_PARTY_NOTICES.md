# Third-party software

The [MIT license](LICENSE) in this repository applies only to EasyZapret itself: the desktop UI, installer and related source code. It does not relicense any other project.

EasyZapret does not include zapret, WinDivert or tg-ws-proxy in this repository. On first run it can download official releases from:

- [Flowseal/zapret-discord-youtube](https://github.com/Flowseal/zapret-discord-youtube)
- [Flowseal/tg-ws-proxy](https://github.com/Flowseal/tg-ws-proxy)

Those packages keep their own licenses. Check the license files that come with each download.

The Windows release installer bundles the pinned, unmodified upstream archives for Xray-core 25.10.15 and sing-box 1.12.12. The VPN page checks their SHA-256 hashes before extraction into the protected component directory. Builds without embedded archives download the same packages. EasyZapret launches the executables as separate processes, and their license files are retained alongside them.

- Xray-core (MPL-2.0). [Source and build files for v25.10.15](https://github.com/XTLS/Xray-core/tree/v25.10.15)
- sing-box (GPL-3.0-or-later). [Corresponding source and build files for v1.12.12](https://github.com/SagerNet/sing-box/tree/v1.12.12)
- The sing-box binary uses Wintun 0.14.1 embedded by sing-tun. [Wintun license](src-tauri/licenses/Wintun-0.14.1.txt). [sing-tun v0.7.3](https://github.com/SagerNet/sing-tun/tree/v0.7.3/internal/wintun) · [Wintun project and downloads](https://www.wintun.net/)

| Component | Source | License |
| --------- | ------ | ------- |
| zapret (winws) | [bol-van/zapret](https://github.com/bol-van/zapret) | MIT |
| WinDivert | [basil00/WinDivert](https://github.com/basil00/WinDivert) | LGPLv3 or GPLv2, as stated by its authors |
| Cloudflare WARP / 1.1.1.1 | [one.one.one.one](https://one.one.one.one/) | Separate official client, not part of this project |

EasyZapret does not modify WinDivert and does not store it in this repository.

Use EasyZapret and any downloaded components at your own risk and in accordance with the laws of your country.
