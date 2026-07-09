<div align="center">

# EasyZapret

**Десктопный менеджер для [zapret-discord-youtube](https://github.com/Flowseal/zapret-discord-youtube) и [tg-ws-proxy](https://github.com/Flowseal/tg-ws-proxy) от FlowSeal**

Современное приложение в стиле VPN-клиента: два больших переключателя — **Zapret** и **Telegram Proxy** — вместо ручного запуска `.bat` и PowerShell.

<br>

## ⬇️ Скачать для Windows

**Установщик лежит не в «Code», а в разделе Releases — откройте его и скачайте файл `EasyZapret_*_x64-setup.exe` из блока Assets.**

<br>

<a href="https://github.com/danyalacio/easyzapret/releases/latest">
  <img src="https://img.shields.io/badge/Скачать_последнюю_версию-Releases-0ea5e9?style=for-the-badge&logo=github&logoColor=white" alt="Скачать EasyZapret" height="48">
</a>

<br>

**[→ Открыть страницу релизов (скачать установщик)](https://github.com/danyalacio/easyzapret/releases/latest)**

*Актуальная версия: [v0.4.0](https://github.com/danyalacio/easyzapret/releases/tag/v0.4.0) · Windows 10/11 x64 · нужны права администратора*

</div>

---

## Возможности

- **Главный экран** — независимые переключатели Zapret и Telegram Proxy (работают одновременно)
- **Стратегии** — выбор готовых стратегий FlowSeal (`general`, ALT1–12, FAKE TLS и др.) из скачанного релиза
- **Сервис** — полный паритет с `service.bat`:
  | Функция service.bat | В EasyZapret |
  |---|---|
  | Install Service | Установка стратегии в автозапуск (служба Windows `zapret`) |
  | Remove Services | Удаление служб zapret / WinDivert / WinDivert14 |
  | Check Status | Статус службы, WinDivert и процесса winws.exe |
  | Game Filter | Режимы off / TCP+UDP / TCP / UDP |
  | IPSet Filter | Режимы none / loaded / any |
  | Auto-Update Check | Флаг `check_updates.enabled` |
  | Update IPSet List | Скачивание актуального `ipset-all.txt` |
  | Update Hosts File | Проверка hosts + открытие файлов для ручного слияния |
  | Run Diagnostics | Все проверки из service.bat: BFE, прокси, TCP timestamps, Adguard, Killer, Check Point, SmartByte, VPN, DoH, конфликтующие обходы, очистка кэша Discord |
  | Run Tests | Свой UI тестов (см. ниже) |
- **Тесты** — обёртка над `utils/test zapret.ps1`: стандартные HTTP/TLS/ping-тесты по `utils/targets.txt` и DPI-чекеры (TCP 16-20 freeze), прогресс и результаты в UI, лог в `C:\EasyZapret\logs\tests.log`, определение лучшей конфигурации
- **Списки** — редактирование `list-general-user.txt`, `list-exclude-user.txt`, `ipset-all.txt`, `ipset-exclude-user.txt` (пользовательские файлы переживают обновления zapret)
- **Telegram Proxy** — запуск/остановка tg-ws-proxy, сервер/порт/secret, «Открыть в Telegram» (`tg://proxy?...`), «Скопировать ссылку»
- **Логи** — вкладки Zapret / Telegram Proxy / Тесты / Система, live tail, очистка, открытие папки
- **Трей** — закрытие окна сворачивает в трей; иконка меняет цвет по статусу; меню: Открыть, Zapret вкл/выкл, TG Proxy вкл/выкл, Выход
- **Обновления** — проверка релизов обоих компонентов на GitHub (по тегам) при старте и вручную; только уведомления, установка по кнопке
- **Темы** — светлая / тёмная / системная; **языки** — русский и английский

## Требования

- **Windows 10/11 (x64)**
- **Права администратора** — приложение всегда запускается elevated (манифест `requireAdministrator`): это нужно для winws.exe, драйвера WinDivert и управления службами
- Интернет при первом запуске (скачивание компонентов FlowSeal)

## Установка

1. Откройте **[страницу релизов](https://github.com/danyalacio/easyzapret/releases/latest)** и скачайте **`EasyZapret_*_x64-setup.exe`** из раздела **Assets** (не из вкладки Code).
2. Запустите установщик.
3. **SmartScreen**: приложение не подписано сертификатом — Windows покажет предупреждение. Нажмите «Подробнее» → «Выполнить в любом случае».
4. При первом запуске EasyZapret предложит скачать последние релизы **zapret-discord-youtube** и **tg-ws-proxy** с GitHub FlowSeal и распакует их в `C:\EasyZapret`.

Приложение устанавливается в Program Files, а все данные (компоненты, списки, логи, настройки) живут в фиксированной папке `C:\EasyZapret` — путь без кириллицы и пробелов, как требует zapret.

### Структура C:\EasyZapret

```
C:\EasyZapret\
├── zapret\           # релиз zapret-discord-youtube (bin\, lists\, general*.bat, utils\)
├── tg-ws-proxy\      # TgWsProxy_windows.exe
├── logs\             # zapret.log, tgproxy.log, tests.log, app.log
├── tmp\              # временные файлы загрузок
└── config.json       # настройки EasyZapret
```

## Антивирус и WinDivert

Zapret перехватывает трафик через драйвер **WinDivert** — антивирусы часто помечают его и `winws.exe` как угрозу. Это **ложное срабатывание** (механика описана в README FlowSeal). Если zapret не запускается или пропадает `WinDivert64.sys`:

1. Добавьте `C:\EasyZapret` в исключения антивируса
2. Переустановите zapret через Настройки → Компоненты → Переустановить

EasyZapret снимает блокировку Mark-of-the-Web (аналог «Разблокировать» в свойствах файла) с распакованных файлов автоматически.

## ⚠️ Фейковые репозитории

Существуют поддельные копии zapret с вредоносным кодом. EasyZapret скачивает релизы **только** из официальных репозиториев:

- https://github.com/Flowseal/zapret-discord-youtube/releases
- https://github.com/Flowseal/tg-ws-proxy/releases

## Подсказки

- **Secure DNS**: для стабильного обхода настройте DNS-over-HTTPS (1.1.1.1 / 8.8.8.8) в Windows 11 или в браузере — подсказка есть в Настройках, проверка в Диагностике.
- **Telegram: не грузятся медиа** — в настройках прокси Telegram удалите записи DC → IP, кроме `4:149.154.167.220` (блок-подсказка на вкладке Telegram).
- **Тесты** нельзя запускать при установленной службе zapret (как и в оригинальном скрипте) — сначала удалите службу.

## Технический стек

| Слой | Технология | Почему |
|---|---|---|
| Backend | **Tauri 2 (Rust)** | Лёгкий рантайм (~10 МБ вместо ~150 МБ у Electron), нативный трей, надёжная работа с процессами (`winws.exe`, `sc`, `reg`, `netsh`) и файловой системой |
| Frontend | **React 18 + TypeScript + Vite** | Быстрый dev-цикл, строгая типизация |
| UI | **Tailwind CSS 4** + собственные компоненты в духе shadcn/ui | Современный классический стиль, темы без лишних зависимостей |
| i18n | **i18next** | RU + EN, все строки через словари |
| Установщик | **NSIS** (Tauri bundler) | perMachine-установка, хук удаления служб zapret/WinDivert при деинсталляции |

Код FlowSeal **не копируется** в репозиторий — релизы скачиваются в рантайме, а GUI воспроизводит поведение `service.bat`, `general*.bat` и `utils/test zapret.ps1` поверх оригинальных файлов.

## Сборка из исходников

```bash
# зависимости: Node.js 18+, Rust (stable), на Windows — WebView2 (есть в Win10/11)
npm install
npm run tauri dev     # разработка
npm run tauri build   # сборка NSIS-установщика (на Windows)
```

Артефакт: `src-tauri/target/release/bundle/nsis/EasyZapret_0.1.0_x64-setup.exe`.

> Примечание: NSIS-установщик собирается на Windows. Манифест администратора задаётся в `src-tauri/build.rs`, хуки деинсталляции — в `src-tauri/installer/hooks.nsh`.

## Структура проекта

```
ZapretUI/
├── src/                  # React frontend
│   ├── pages/            # Главная, Стратегии, Сервис, Тесты, Списки, Telegram, Логи, Настройки
│   ├── components/       # UI-кит, сайдбар, модалки
│   ├── i18n/             # ru / en
│   └── lib/              # API-обёртки, store, типы
├── src-tauri/            # Rust backend (Tauri)
│   ├── src/
│   │   ├── zapret/       # стратегии, winws, службы (service.bat-логика), диагностика
│   │   ├── tg_proxy/     # управление TgWsProxy
│   │   ├── updates/      # GitHub releases: проверка и установка
│   │   ├── tests/        # стандартные тесты + DPI-чекеры
│   │   └── logs/         # чтение/запись логов
│   └── installer/        # NSIS-хуки
├── assets/               # логотип
└── README.md
```

## Roadmap

- **v0.1** *(текущая)* — два переключателя, стратегии, service.bat-паритет, тесты, списки, tg-ws-proxy, логи, трей, уведомления об обновлениях, NSIS-установщик
- **v0.2+** — редактор стратегий (создание и изменение собственных конфигураций winws)
- **v0.3+** — интеграция Cloudflare WARP (сейчас — заглушка «Скоро» в меню)

## Благодарности

- **[FlowSeal](https://github.com/Flowseal)** — за zapret-discord-youtube и tg-ws-proxy
- **[bol-van](https://github.com/bol-van)** — за оригинальный [zapret](https://github.com/bol-van/zapret) и winws
- [hyperion-cs/dpi-checkers](https://github.com/hyperion-cs/dpi-checkers) — набор DPI-чекеров, используемый в тестах

---

*Личный проект. EasyZapret не связан с FlowSeal. Используйте на свой страх и риск.*
