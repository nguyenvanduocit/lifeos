# The iPhone app

Apple Health keeps its data on the iPhone and the Watch; a Mac has no Health store, so LifeOS cannot read it
itself. The app in `apps/ios` reads Apple Health on the phone and sends one value per day to LifeOS, and shows
the web app in its first tab.

| Measure | Metric in LifeOS | How a day is counted |
|---|---|---|
| Steps | `bước chân` (bước) | sum |
| Walking and running distance | `quãng đường` (km) | sum |
| Sleep (core, deep, REM) | `giấc ngủ` (giờ) | the night that ended that day; phone and Watch overlaps counted once |
| Weight | `cân nặng` (kg) | last weighing |
| Resting heart rate | `nhịp tim nghỉ` (bpm) | average |
| HRV (SDNN) | `hrv` (ms) | average |
| Active energy | `năng lượng hoạt động` (kcal) | sum |
| Exercise | `phút tập` (phút) | sum |

The values land in `metrics` with `source = 'apple-health'` and the note `Apple Health`, beside what the owner logs
by hand. A day sent again replaces the phone's value for it, never a hand-logged one.

## When it syncs

- When the app opens, and when you tap **Đồng bộ ngay**.
- When HealthKit has new steps, sleep, weight or resting heart rate (background delivery, at most hourly).
- On a background refresh every few hours, for wakes iOS dropped.

Each sync sends every day since the day before the last one LifeOS kept, at least the last 7 days; the first
sends the last 365. HealthKit cannot be read while the phone is locked, so background wakes then do nothing and
the next unlocked one catches up.

## Server

`POST /api/health/apple` on the web server, body `{"readings": [{"day": "2026-11-25", "measure": "steps",
"value": 8123}]}`. Like the other owner requests it has no login: it requires the header `x-lifeos-account: 1` and
refuses cross-site browser requests. Keep it on loopback or the tailnet; the phone must be on the tailnet.

## Build and install

Needs Xcode (not only the Command Line Tools), XcodeGen (`brew install xcodegen`), your Apple ID in Xcode →
Settings → Accounts (a free one installs on your own phone; its profile expires after 7 days), and the iPhone
plugged in, unlocked, trusting this Mac and in Developer Mode (Settings → Privacy & Security). Then:

```sh
make ios        # scripts/install-ios.sh: generate the project, build, install, launch
```

The first time, iOS may ask you to trust the developer in Settings → General → VPN & Device Management. On first
launch allow the Health categories. The server address defaults to `https://lifeos.aiocean.dev`; change it in the
**Sức khoẻ** tab. To work on the app in Xcode: `cd apps/ios && xcodegen generate && open LifeOS.xcodeproj`.

The **LifeOS** tab is the web app in a WebView. The web app talks to the server over WebTransport with a pinned
certificate; if the WebView on your iOS version cannot open that link, use Health sync from the app and the web app
from Safari on a Mac.
