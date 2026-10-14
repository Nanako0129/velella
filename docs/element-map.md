# 訊息區元素對照表

## 文件目的

這份文件把官方 Discord 訊息區的元素一個一個拆開，記下 Swiftcord v2 對應元素的樣子，以及 velella 的現況，最後決定 velella 每個元素要取哪一方的樣式。依照既有決定，訊息內容的版面跟官方 Discord（T1），選單、操作列、popover 這類外框元件跟 v2 的原生與玻璃風格（[ui-design.md](ui-design.md)）。第一版只涵蓋第一批三個元素：右鍵選單、操作列、文字選取；其餘元素等第二批截圖再補。

> **注意：** 截圖都在 `refs/`（`refs/discord-ground-truth/captures/e0-*.png`、`refs/swiftcord-v2-screens/d-*.png`），只在本機、不進 git。v2 binary 的拆解細節在 `refs/swiftcord-v2-teardown.md`，同樣不進 git；這份文件只寫看得到的行為。

## 資料來源

| 來源 | 內容 |
|---|---|
| 官方 Discord 截圖 | 2026-10-15 自動截取（velella 開發討論串，深色，英文介面）。這台 Discord 裝了 Vencord，操作列的 `<>`（ViewRaw）是 Vencord 加的 |
| Discord CSS | 本機 Discord 快取的 `.css`：操作列模組 `f84418`、選單模組 `c1e9c4`、訊息模組 `c19a55` |
| v2 截圖 | 同一個討論串，Swiftcord 1.7.2，2026-10-15 自動截取 |

## 右鍵選單

| 項目 | 官方 Discord | Swiftcord v2 |
|---|---|---|
| 外觀 | 自繪深色選單：每列左側有圖示，最上方一排 4 個最近用過的 reaction，子選單有 `›`，危險項目紅字 | macOS 原生選單（玻璃），純文字無圖示，沒有快速 reaction 列，Delete 不是紅字 |
| 別人的訊息 | 快速 reaction／Add Reaction › ｜ Reply、Forward ｜ Copy Text、Pin Message、Bookmark Message、Create Reminder ›、Apps ›、Mark Unread、Copy Message Link、Speak Message ｜ Delete Message、Report Message ｜ Copy Message ID | Add Reaction ｜ Reply、Forward ｜ Copy Text、Pin Message、Mark Unread、Copy Message Link、Speak Message、Translate ｜ Delete Message ｜ Copy Message ID |
| 自己的訊息 | 多 Edit Message（Reply 前），沒有 Report | 多 Edit（Reply 前） |
| 連結上 | 訊息選單再加 Copy Link、Open Link ↗ | 跟訊息選單一樣，沒有連結項目 |
| 圖片上（這次是 embed 圖，本身是連結） | 同連結 | 跟訊息選單一樣 |
| 「…」按鈕 | 打開同一個選單 | 打開同一個選單 |
| CSS（c1e9c4） | 列高至少 32、內距 8、項目圓角 2、hover 底色 `--background-mod-subtle`、分隔線 1 px `--border-subtle` 上下 8、危險字色 `--text-feedback-critical`、子選單寬 188-320；選單寬度與陰影沒找到 | — |
| Delete、Report、Pin 是否出現 | 依權限（這個帳號在該伺服器有管理權限） | 同 |
| velella 現況 | 沒有 | — |

> **注意：** 官方的 Copy Text 複製的是原始 markdown（含 `||暴雷||`）還是畫面上的文字，還沒確認：要實際點選單項目才測得到，這次只開選單、沒有點。

> **注意：** 附件圖片（不是 embed）的右鍵選單還沒截到，那個討論串沒有附件。Discord 通常會多 Copy Image、Save Image，要等第二批確認。

## 操作列（滑鼠停在訊息上）

| 項目 | 官方 Discord | Swiftcord v2 |
|---|---|---|
| 位置 | 訊息右上角，浮在上一則訊息的底邊上 | 同 |
| 外觀 | 深色圓角框（radius 8、1 px `--border-muted`、`--shadow-low`、內距 2），按鈕 radius 6、圖示 20×20、hover 放大 1.1；分隔線 1×24 | 玻璃膠囊 |
| 別人的訊息 | 3 個快速 reaction ｜ `<>`（Vencord）、emoji、回覆、轉發、… | 3 個快速 reaction、emoji、回覆、轉發、… |
| 自己的訊息 | 回覆前多一個鉛筆（編輯） | 回覆換成鉛筆（編輯） |
| hover 中的訊息列 | 整列底色變淺（CSS 沒找到對應規則，是截圖觀察） | 整列底色變淺 |
| velella 現況 | 沒有；只有接續訊息左側的 hover 時間（T1） | — |

## 文字選取

| 項目 | 官方 Discord | Swiftcord v2 |
|---|---|---|
| 訊息內選字 | 可以（`user-select: text`，c19a55） | 可以 |
| 跨訊息拖曳 | 可以，作者名和時間也會被選進去 | 不行：選取停在起點那則訊息的結尾 |
| 選取顏色 | 品牌色半透明（全域 `::selection`） | 系統選取色 |
| velella 現況 | 不能選：段落是 SwiftUI `Text` 加自訂 `TextRenderer`，沒有開 `textSelection`；只有程式碼區塊有 Copy 按鈕 | — |

## velella 的取捨（待使用者決定）

| 元素 | 建議 | 理由 |
|---|---|---|
| 右鍵選單 | v2 樣式（SwiftUI `.contextMenu`，原生玻璃選單、純文字），項目順序跟 v2。連結上多 Copy Link、Open Link（官方有、v2 沒有） | 外框元件跟 v2 的決定；原生選單免費得到鍵盤操作與無障礙 |
| 選單項目 | 只放已經能用的項目：這一輪是 Copy Text、Copy Message Link、Copy Message ID，連結上加 Copy Link、Open Link。其餘（回覆、編輯、刪除、釘選、reaction…）等功能做出來才加進選單 | 不放按了沒反應的項目 |
| 操作列 | v2 的玻璃膠囊，等 M3 有回覆、reaction、編輯時再做；這一輪先不做 | 現在放上去只有「…」一個按鈕有用 |
| 文字選取 | 先不做拖曳選取，這一輪靠 Copy Text | SwiftUI 的 `textSelection` 只能在單一 `Text` 內選，velella 一個段落一個 `Text`，範圍比 v2 的「一則訊息」還小；還要處理被遮住的暴雷會不會被選到。要做到 v2 的程度，得把訊息改成 NSTextView 渲染，是一個獨立切片 |
