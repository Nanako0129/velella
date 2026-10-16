# velella Roadmap

## 文件目的

這份文件列出 velella 從第一次登入到 iOS 版的里程碑順序。目標是完整的 Discord 客戶端，功能與行為對齊官方桌面客戶端（使用者 2026-10-16）；每個功能群組的狀態與所屬波次見 [parity.md](parity.md)。外觀仍參考 Swiftcord v2（Liquid Glass、iPad）。只有進行中的里程碑有詳細計畫；後面的里程碑等前一個完成、Rust API 成形之後才寫計畫，每份計畫都經過審查與核准才開工。參考專案的分析見 [reference-analysis.md](reference-analysis.md)。

## 里程碑

| 里程碑 | 目標 | 最大風險 | 主要參考 |
|---|---|---|---|
| M1 登入與側欄（進行中，[計畫](plans/m1.md)） | WKWebView 登入、伺服器與頻道清單、DM 清單 | user account READY 形狀只能靠真實連線驗證 | Serein gateway、Concord READY parser |
| M2 讀訊息 | 訊息時間軸、歷史分頁、即時新增／編輯／刪除、未讀標記、Discord markdown（mention、自訂 emoji、spoiler、code block、時間戳）、embed 與附件基本渲染 | 訊息列表捲動體驗：M2 第一步做 spike，比較 SwiftUI `LazyVStack` 與 NSTableView | matrix VectorDiff 時間軸、textual fork（MIT）、Swiftcord 舊版 embed／附件 view |
| M3 送訊息與互動 | 輸入框、nonce 樂觀插入、編輯／刪除、回覆、reaction、typing、附件上傳、mention 自動完成、per-route REST 限流 | rate limit 與 captcha | Concord `commands.rs`、`rest.rs` |
| M4 擴充系統（插件） | 使用者用腳本擴充 velella（暫定 Luau，跑在 Rust 引擎裡，M4 開頭 spike 定案）、capability 授權、宣告式 UI 元件（badge、選單項目、設定面板）由 Swift 繪製、代表帳號送出的動作須使用者確認或預設關閉、Vencord 功能以隨附 plugin 的形式移植 | plugin API 一旦公開就要維持相容；自動送出的 plugin 等同 self-bot | Neovim／WezTerm 的 Lua 擴充模式、Serein `docs/extensions.md`（capability 與 catalog 設計）、Vencord（GPL-3.0，功能清單） |
| M5 伺服器深度 | 成員列表（op 37）、presence、使用者 popover 與個人檔案、thread／forum、系統通知、SQLite 訊息快取 | lazy 成員列表協定 | Serein op 37 與 SQLite、Concord 權限 |
| M6 語音與視訊 | 語音頻道、DAVE E2EE、DM 通話，之後是螢幕分享（ScreenCaptureKit + VideoToolbox） | DAVE 實作沒有密碼學稽核；songbird 不支援 DM 通話 | Concord voice（GPL）、davey |
| M7 打磨與發佈 | 對照 Swiftcord v2 畫面的 Liquid Glass 設計、設定、多帳號、原生 QR 登入、Developer ID 簽章與公證、Sparkle、data-protection keychain | 視覺需要多輪收斂 | Swiftcord v2 畫面、TokenBar 的 Sparkle 與簽章流程 |
| M8 iOS / iPadOS | XCFramework 打包流程、iPad UI | 發佈管道：非官方 client 過不了 App Review，GPL 與 App Store 條款衝突 | matrix `xtask/src/swift.rs` |

## 排序理由

聊天在前、語音在後：語音依賴 gateway、權限與成員資料，這些在 M1 到 M5 才會到位。擴充系統排在 M3 之後（使用者 2026-10-13 決定）：最常見的 plugin 只需要訊息讀寫，訊息模型穩定之後就能公開 API，太早做會讓 plugin 跟著 core 一起壞。腳本語言傾向 Lua（charliie 的提議）而非 Wasm：目標是讓使用者自己寫功能，一個 `.lua` 檔的門檻遠低於 Wasm 工具鏈。M2 與 M3 的計畫要把 plugin 掛點當設計條件（訊息顯示前、右鍵選單、送出前、slash 指令、設定面板），M4 才只需要開放而不必回頭改 core。M2 的訊息列表決定整個 client 用起來順不順，Element X 就曾因為分頁時捲動跳動，把 timeline 從 SwiftUI 改成 UITableView，所以 M2 先做 spike，再依結果寫詳細計畫。

## Vencord 功能移植（初步分類）

Vencord 的 plugin 靠 regex patch Discord 網頁版的 webpack 程式碼運作，在原生 client 上無法直接執行，只能逐一用原生方式重做功能。Vencord 是 GPL-3.0-or-later，與 velella 的 GPL-3.0-only 相容。2026-10-13 對 `refs/vencord/src/plugins` 的 168 個目錄做了第一輪分類（只讀 plugin 名稱、說明與入口檔，結果仍需逐項確認）：

| 分類 | 數量 | 意思 |
|---|---|---|
| 內建功能 | 77 | 普遍有用，直接做進 velella |
| 隨附插件 | 17 | 小眾、使用者自選，M4 之後以插件提供 |
| 原生不需要 | 54 | 只為修改網頁版或 Electron 的 UI、CSS、遙測而存在 |
| 做不到 | 8 | Nitro 偽造、Discord 內部實驗旗標、client 拿不到的資料、繞過防護 |
| 需外部服務 | 12 | 翻譯、Spotify、Last.fm 等第三方 API |

> **注意：** 第一輪把 showHiddenChannels 歸成「做不到」，但它其實是 patch 掉 client 端的 `VIEW_CHANNEL` 檢查（`refs/vencord/src/plugins/showHiddenChannels/index.tsx:85`），代表 client 收得到這些頻道，已改列為隨附插件。M1 的側欄預設仍隱藏沒有權限的頻道。

價值最高、最早可做的幾項：

| Plugin | 功能 | 最早里程碑 |
|---|---|---|
| blurNsfw | NSFW 附件預設模糊 | M2 |
| noBlockedMessages | 完全隱藏封鎖對象的訊息 | M2 |
| imageZoom、fixImagesQuality | 圖片放大、載入原始解析度 | M2 |
| showMeYourName、roleColorEverywhere | 顯示 username、身分組顏色 | M2 |
| clearURLs | 送出時去掉追蹤參數 | M3 |
| silentMessageToggle、silentTyping | 靜音送出、不送 typing | M3 |
| typingIndicator、previewMessage | 頻道 typing 提示、送出前預覽 | M3 |
| betterFolders、pinDms | 資料夾側欄、DM 置頂 | M5 |
| onePingPerDM、notificationVolume | 通知音控制 | M5 |

messageLogger（保存別人已刪除或編輯的訊息）價值高，但牽涉他人隱私，只能做成預設關閉、只存本機的隨附插件。

## 延後的決策

| 決策 | 何時決定 |
|---|---|
| iOS 發佈管道（sideload、歐盟替代市集或其他） | M8 開始前 |
| 是否做離線 demo backend 給 SwiftUI Preview 用（scope 的做法） | M2 做 UI 時 |
