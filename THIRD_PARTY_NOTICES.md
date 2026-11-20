# Third-party software

The [MIT license](LICENSE) in this repository applies only to EasyZapret itself: the desktop UI, installer and related source code. It does not relicense any other project.

EasyZapret does not include zapret, WinDivert or tg-ws-proxy in this repository. On first run it can download official releases from:

- [Flowseal/zapret-discord-youtube](https://github.com/Flowseal/zapret-discord-youtube)
- [Flowseal/tg-ws-proxy](https://github.com/Flowseal/tg-ws-proxy)

Those packages keep their own licenses. Check the license files that come with each download.

The VPN page downloads pinned, checksum-verified official releases of Xray-core 25.10.15 and sing-box 1.12.12 into the protected component directory. Their license files are retained alongside the executables. Xray-core uses MPL-2.0 and sing-box uses GPL-3.0-or-later. sing-box manages its Windows TUN interface through Wintun, whose license is included in its upstream source. EasyZapret launches these components as separate processes. [Xray-core](https://github.com/XTLS/Xray-core) · [sing-box](https://github.com/SagerNet/sing-box) · [Wintun](https://www.wintun.net/)

| Component | Source | License |
| --------- | ------ | ------- |
| zapret (winws) | [bol-van/zapret](https://github.com/bol-van/zapret) | MIT |
| WinDivert | [basil00/WinDivert](https://github.com/basil00/WinDivert) | LGPLv3 or GPLv2, as stated by its authors |
| Cloudflare WARP / 1.1.1.1 | [one.one.one.one](https://one.one.one.one/) | Separate official client, not part of this project |

EasyZapret does not modify WinDivert and does not store it in this repository.

Use EasyZapret and any downloaded components at your own risk and in accordance with the laws of your country.
