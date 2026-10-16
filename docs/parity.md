# velella 功能對照表

## 文件目的

這份文件把官方 Discord 桌面客戶端的每一項功能（來源是 [docs/discord-features.md](discord-features.md) 的 1,902 列調查），逐項對照 velella 目前的狀態與預計交付的排程，用來避免「功能有沒有做齊」要靠使用者追問。每個功能群組列出官方行為摘要、velella 狀態、負責的排程（`docs/plans/menu.md` 的切片 ID、`selection N2` / `W1`，或 `未排`）與備註。每一波結束時更新一次；它是判斷「什麼算完成」的唯一依據，狀態以 `velella-status`（2026-10-16，分支 `sel-int`）的盤點為準，沒有明確提到的項目一律記為 `未做`。

## 目錄

1. [Account & login](#account--login)
2. [Server rail (guild sidebar)](#server-rail-guild-sidebar)
3. [Channel list](#channel-list)
4. [DMs and friends](#dms-and-friends)
5. [Message display](#message-display)
6. [Message actions](#message-actions)
7. [Composer](#composer)
8. [Reactions](#reactions)
9. [Search, Inbox & pins](#search-inbox--pins)
10. [Member list & profiles](#member-list--profiles)
11. [Presence & status](#presence--status)
12. [Notifications](#notifications)
13. [Voice & video](#voice--video)
14. [Threads and forums](#threads-and-forums)
15. [User Settings](#user-settings)
16. [Server settings & moderation](#server-settings--moderation)
17. [Keyboard shortcuts](#keyboard-shortcuts)
18. [App-level features](#app-level-features)
19. [Nitro, monetization & extras](#nitro-monetization--extras)
20. [Appendix: 2024–2026 feature timeline (official changelogs and patch notes)](#appendix-20242026-feature-timeline-official-changelogs-and-patch-notes)
21. [摘要](#摘要)

## 狀態圖例

| 狀態 | 意義 |
|---|---|
| 完成 | 入口、情境、狀態與即時同步都與官方一致（依 CLAUDE.md 的完整度標準） |
| 部分 | 有一部分可用，缺少的部分寫在備註 |
| 唯讀 | 能顯示或讀取，但不能操作或修改 |
| 未做 | 完全沒有，或盤點沒有提到 |
| macOS 無此功能 | 官方 macOS 桌面客戶端本身沒有（Windows 專屬、行動版專屬或官方根本沒有），不需要對齊 |
| 不做（需使用者決定） | 官方有但打算不做；依 CLAUDE.md 必須由使用者用 ASK 決定，目前沒有任何項目屬於這一類 |

排程欄的值：`M0`、`F1`、`F2`、`P1`、`J`、`A`、`B`、`C`、`D`、`E`、`I`、`G`、`H` 是 `docs/plans/menu.md` 的切片；`selection N2` 與 `W1` 是 `docs/plans/selection.md` 的切片；`未排` 表示目前沒有任何計畫涵蓋。同一列出現多個 ID 表示由多個切片共同交付；括號內是該切片涵蓋的部分。

## Account & login
| 功能群組 | 官方行為摘要 | velella 狀態 | 排程 | 備註 |
|---|---|---|---|---|
| 註冊 | 桌面/瀏覽器欄位：email、使用者名稱、密碼、生日，可能出 CAPTCHA；可由邀請連結開始；行動版可用手機號碼 | 未做 | 未排 | 登入頁以內嵌網頁處理，原生註冊流程未做 |
| 密碼與電話登入 | email 或電話加密碼登入、忘記密碼重設、密碼規則（至少 8 字元） | 部分 | 未排 | 內嵌 Discord 網頁登入（WKWebView）可走這些流程，無原生表單 |
| QR code 登入 | 登入畫面顯示 QR code，用已登入的行動版掃描 | 部分 | 未排 | 內嵌網頁的 QR 可用；原生 QR 登入未做 |
| Passkey 與安全金鑰登入 | Passkey 無密碼登入、安全金鑰作為第二因素 | 部分 | 未排 | 是否在 WKWebView 內可用未驗證 |
| MFA 登入挑戰 | 6 位數驗證碼、「用其他方式驗證」 | 部分 | 未排 | 內嵌網頁處理 |
| CAPTCHA 與登入安全驗證 | 註冊、電話驗證、改 email 時的 CAPTCHA；新登入位置驗證；購買授權信 | 部分 | 未排 | CAPTCHA 由內嵌網頁處理；新登入位置驗證為 unconfirmed |
| 臨時帳號（Provisional accounts） | 在整合 Discord 的遊戲內使用語音/文字，無需完整帳號 | 未做 | 未排 | 偏遊戲 SDK 情境 |
| MFA 設定 | 設定位置（My Account > Password and Authentication）；註冊安全金鑰/Passkey 與移除；啟用/移除驗證器 App（TOTP）；SMS 備援；備份碼；敏感操作需 MFA；鎖定復原；iOS 顯示密碼按鈕 | 未做 | 未排 | |
| 電子郵件與電話驗證 | Email 驗證、電話驗證（不收 VoIP）、新增/變更/移除電話（每帳號一組） | 未做 | 未排 | |
| 帳號憑證變更 | 變更 email、email 變更復原、變更密碼、變更使用者名稱（2–32 字元規則） | 未做 | 未排 | |
| 顯示名稱 | 1–32 字元、可含 emoji，被伺服器/好友暱稱覆蓋 | 未做 | 未排 | 目前只顯示他人的顯示名稱，無自己的編輯入口 |
| 年齡組別與青少年保護 | Account Status 頁、Confirm Age/Retry、青少年預設保護（敏感內容過濾、訊息請求強制） | 未做 | 未排 | Teen-by-default 為 unconfirmed |
| 帳號切換 | 頭像選單 Switch Accounts，桌面最多同時 5 個帳號；切換時部分偏好重設、草稿捨棄 | 未做 | 未排 | 目前單帳號 |
| 登出 | User Settings 紅色 Log Out；登出時語音訊息停止播放 | 部分 | 未排 | 僅本機登出（Sign Out 選單），不撤銷 session |
| 裝置與登出所有裝置 | Devices 頁（2026 年 6 月）列出已登入 session；改密碼登出所有裝置 | 未做 | 未排 | Devices 頁為 unconfirmed |
| 停用、刪除與閒置刪除 | Disable Account、Delete Account（先轉移伺服器、需密碼/MFA）、兩年未使用的帳號可能被排程刪除 | 未做 | 未排 | |
| 帳號狀態與違規 | Account Standing（等級）、違規與申訴（系統 DM 摘要卡）、警告（不影響 standing） | 未做 | 未排 | |
| 被駭帳號處理 | 檢查已授權 App（Deauthorize）、重設密碼、啟用 MFA | 未做 | 未排 | |

## Server rail (guild sidebar)
| 功能群組 | 官方行為摘要 | velella 狀態 | 排程 | 備註 |
|---|---|---|---|---|
| Home 與 DM 入口 | 頂端 Home / Direct Messages 按鈕，開啟 DM 列表與 Friends；從伺服器按 Ctrl+1 回到 Home | 部分 | 未排 | 有 DM 入口與未讀總數，沒有 Home 頁（status：missing Home） |
| Favorites 伺服器圖示與檢視 | Nitro 實驗（2026）；收錄文字/語音/論壇頻道與 DM（不含討論串），可自建分類 | 未做 | 未排 | 實驗功能 |
| 伺服器圖示與提示 | 方形圓角圖示、動態圖示需 Boost Level 1；hover 顯示名稱與類型徽章（Community、Discoverable、Partnered、Verified）；選取/hover 指示條 | 部分 | 未排 | 圖示、選取、hover 條完成；類型徽章 tooltip 未做 |
| 未讀與提及標記 | 未讀指示、紅色提及計數（@username、@role、@everyone、@here）、靜音伺服器狀態（無未讀，仍顯示提及） | 完成 | 未排 | 未讀指示的靜音細節為 unconfirmed |
| 伺服器重新排序 | 拖曳排序（資料夾同理）；鍵盤排序（Ctrl/Cmd+D 舉起、方向鍵移動、Space/Enter 放下） | 未做 | 未排 | 目前排序來源未追蹤（status：partial） |
| 伺服器導覽快捷鍵 | Cmd+Option+Up/Down 切換伺服器；Shift+Esc 標記伺服器已讀 | 未做 | 未排 | 另見 Keyboard shortcuts |
| 側欄捲動與語音浮窗 | 側欄可捲動、底部 Discover 完全可達（2026 修正）；通話中伺服器側欄的語音 popover | 部分 | 未排 | 可捲動；語音 popover 為 unconfirmed 且語音未做 |
| Student Hubs 入口 | 左下角 Explore Public Servers，找學校的 Student Hub | 未做 | 未排 | |
| 側欄底部其他入口 | Download Apps 按鈕、Nitro / Shop 入口（位於 Home 側欄而非 rail） | 未做 | 未排 | 兩項皆 unconfirmed；桌面版無下載 App 的意義 |
| 資料夾顯示與展開 | 資料夾預覽、展開/收合（螢幕閱讀器朗讀展開狀態）、資料夾通知徽章、顏色 | 完成 | 未排 | 資料夾於 READY 後不更新（status：partial 的一部分），展開動作 unconfirmed |
| 資料夾建立與編輯 | 拖曳伺服器到另一個伺服器上停留後建立；無障礙動作建立；拖入/拖出；每個資料夾最多 100 個伺服器；資料夾不能合併；行動版長按移動 | 未做 | 未排 | |
| 資料夾選單與設定 | 右鍵 Folder Settings、Mark Server Folder as Read；設定對話框可改名與顏色（id、name、color） | 未做 | 未排 | 設定對話框為 unconfirmed |
| 新增伺服器 | 底部「+」按鈕（Ctrl/Cmd+Shift+N）；Create My Own（For a club or community / For me and my friends）；範本建立與範本連結預覽；邀請碼欄位；Quick Switcher 貼邀請連結加入 | 未做 | 未排 | |
| 加入限制與邀請狀態 | 伺服器數上限（100／Nitro 200）對話框、Discovery 加入時的邀請暫停 toast、訪客邀請 | 未做 | 未排 | |
| Discover | 底部 Discover 圖示；Servers（分類列）、Quests（最近 30 天、Accept Quest、已領取）、Apps 目錄；舊版 Server Discovery（compass）；伺服器預覽（lurk）；Server Game Shop | 未做 | 未排 | |
| 伺服器右鍵選單：已讀與邀請 | Mark As Read、Invite People（最近 DM/好友、可搜尋） | 未做 | 未排 | status：Invite People 為 stub |
| 伺服器右鍵選單：通知與隱私 | Mute Server（15 分、1/3/8/24 小時、直到我開啟）、Notification Settings、Hide Muted Channels、Show All Channels（unconfirmed）、Privacy Settings（私訊、訊息請求、活動狀態） | 未做 | 未排 | |
| 伺服器右鍵選單：設定與個人檔案 | Edit Per-server Profile（頭像/橫幅（Nitro）、About Me、主題、頭像裝飾）、Server Settings、Server Profile 捷徑 | 未做 | 未排 | Server Settings 為 stub |
| 伺服器右鍵選單：建立 | Create Channel、Create Category、Create Event | 未做 | 未排 | |
| 伺服器右鍵選單：管理 | App Directory、Security Actions（暫停邀請/私訊）、Server Boost、Server Insights、Linked Roles | 未做 | 未排 | |
| 離開伺服器與複製 ID | Leave Server（確認對話框，擁有者需先轉移）、Copy Server ID / Copy Server Info 子選單（需 Developer Mode）、選單在底部自動重新定位 | 未做 | 未排 | Leave Server 確認為 unconfirmed |
| 伺服器標頭與選單 | 頂端伺服器名稱與下拉箭頭開啟伺服器選單；Live Stage 主題顯示 | 部分 | 未排 | 標頭存在，選單項為 stub |
| 伺服器橫幅與徽章 | 橫幅（Level 2 靜態、Level 3 動態）、類型徽章（含 tooltip）、Boost 徽章（點擊開 boost 頁）、Boost 進度條、自訂邀請背景（Level 1）、Server Boosts 分頁（管理員） | 未做 | 未排 | |
| Server tags 顯示 | 4 字元標籤加徽章顯示在聊天、成員列表、個人檔案預覽（2026）；點擊開伺服器個人檔案卡（Join Server / Apply）；報告視窗內不可點 | 未做 | 未排 | |
| Server tags 採用與管理 | 使用者在 Profiles 選擇/移除；採用畫面（2026）；AutoMod 隔離旗標；解鎖需 3 次 boost；顏色由管理員設定；coachmark；舊 Guilds 實驗已結束 | 未做 | 未排 | |

## Channel list
| 功能群組 | 官方行為摘要 | velella 狀態 | 排程 | 備註 |
|---|---|---|---|---|
| 分類顯示與隱藏頻道 | 分類群組頻道（每分類上限 50 個頻道）；無 View Channels 權限的頻道直接隱藏而非鎖定 | 完成 | 未排 | 隱藏頻道已過濾 |
| 分類收合 | 收合後只顯示有未讀的頻道，通知照常；Collapse All Categories（unconfirmed） | 未做 | 未排 | |
| 分類管理 | Mute category（未讀仍顯示）、Edit Category（Overview、Permissions）、拖曳進分類時的權限同步提示（Synced 狀態） | 未做 | 未排 | |
| 頻道重新排序 | 拖曳排序（Manage Channels、拖曳自動捲動）；鍵盤排序（Ctrl/Cmd+D、方向鍵、Space/Enter、Esc）；移動時的「Uncategorized」目標 | 未做 | 未排 | |
| 頻道列表外觀與導覽 | 可調寬度（2025，拖曳邊緣）；個人 Pin channels to top（2026）；新頻道紅色 NEW pill；Alt+Up/Down 切頻道（含只看未讀、未讀含提及）；未讀跳轉 pill | 未做 | 未排 | 未讀跳轉 pill 為 unconfirmed；velella 側欄寬度是否可調未列於 status |
| 文字頻道 | GUILD_TEXT（0），需 View Channel、Send Messages、Read Message History；# 圖示 | 完成 | 未排 | |
| 公告頻道 | GUILD_ANNOUNCEMENT（5），喇叭圖示，Community 限定；Follow 按鈕、Publish 按鈕（每小時 10 則） | 部分 | 未排 | 頻道列出並可讀；Follow 與 Publish 未做（見下方 Announcement 群組） |
| 語音頻道列 | GUILD_VOICE（2），喇叭圖示，鎖頭表示無 Connect 權限；Text chat in voice（Open Chat 泡泡，關閉時最多 3 則預覽）；Living Room 實驗 | 部分 | 未排 | 列出與在線者，無連線；Open Chat 未做；Living Room 為實驗功能 |
| Stage 頻道列 | GUILD_STAGE_VOICE（13），Community 限定；進行中的 Stage 置頂 | 部分 | 未排 | 只有頻道列 |
| 論壇與媒體頻道列 | GUILD_FORUM（15）僅討論串；GUILD_MEDIA（16）類相簿貼文（beta，需 Server Subscriptions）；Directory（14，Student Hubs） | 部分 | 未排 | 論壇/媒體只有頻道列；Directory 未做 |
| 特殊用途頻道 | Rules、Community Updates、Safety notifications、已移除的 Store 頻道（type 6）、Resource pages（Server Guide 資源頁）、LFG（2026 實驗，限 ARC Raiders 官方伺服器） | 未做 | 未排 | Store 移除細節與 Rules 頻道為 unconfirmed |
| 私人、年齡限制與劇透頻道 | 私人頻道鎖頭徽章（Edit Channel > Permissions）；Age-restricted 閘門；Spoiler channel（2026）；訂閱制 Premium 頻道（Server Subscription 層級） | 部分 | 未排 | 私人鎖頭完成；年齡限制閘門、劇透頻道、Premium 頻道未做 |
| 唯讀與僅討論串保護 | 無法發言的頻道顯示 chat guard 或轉向 | 未做 | 未排 | |
| 頻道名稱與混淆 | 頻道名僅接受 Unicode emoji（2026，無 Add Emoji 按鈕）；看不到的頻道被混淆或省略（2026-11-16 API） | 未做 | 未排 | |
| 頻道未讀與提及狀態 | 未讀高亮、紅色提及計數（螢幕閱讀器朗讀）、選取高亮、靜音淡化；未讀行為設定（全部訊息或僅提及） | 完成 | 未排 | Unread state 與 Unread behavior setting 為 unconfirmed；velella 為未讀點與提及徽章 |
| 頻道 hover 動作與提示 | hover 顯示 Create Invite 與 Edit Channel（齒輪）、使用者上限徽章；Slowmode 提示；Activity 圖示 | 未做 | 未排 | |
| 語音頻道側欄 | 頻道下巢狀的連線者列表（收合時顯示使用者）；每人狀態圖示（靜音、拒聽、伺服器靜音、LIVE、視訊、活動）；使用者上限徽章；通話計時（2025）；Chat 與 Invite to Voice 按鈕 | 部分 | 未排 | 只有在線者列表，無說話/靜音狀態；狀態圖示為 unconfirmed |
| 語音頻道管理 | Channel status（最多 500 字加 emoji）、拖曳成員到其他語音頻道（Move Members）、AFK 頻道、從列表結束 Stage | 未做 | 未排 | |
| 公告 Follow 與 Publish | Follow 對話框（選目的地伺服器/頻道）、Publish 按鈕（自己的訊息需 Send Messages）；跨頻道發布訊息的呈現（webhook 以來源伺服器命名）；CHANNEL_FOLLOW_ADD 系統訊息 | 未做 | 未排 | 與 Message display 的系統訊息交叉 |
| Stage 流程 | Start Stage 對話框（需主題、通知伺服器、Create Event）；講者在上、聽眾在下的版面、等待音樂、舉手；容量（音訊 10,000，視訊 50/150/300 加每額外 boost 30）；STAGE_START/END/SPEAKER/TOPIC 系統訊息 | 未做 | 未排 | 需語音連線 |
| 排程活動：建立 | 地點類型（Stage、語音 99 人、外部）；表單（名稱 1–100、說明 1–1000、起訖時間、封面）；重複活動（每日/週/月與週間組合）；發布前預覽；事件上限 100 | 未做 | 未排 | |
| 排程活動：參與 | Interested（RSVP）與人數按鈕；Event Details 視窗（封面、資訊/有興趣的使用者分頁、位置）；Live Now 卡片（Join 或 Go to Channel）；分享事件連結（需 Create Invite）；受限頻道內的事件只對有權者可見 | 未做 | 未排 | |
| 排程活動：管理 | Start / Edit / Cancel（省略號選單、Manage Events/Create Events）、狀態（Scheduled、Active、Completed、Canceled）、Mute New Events | 未做 | 未排 | |
| 頂端入口：Events 與 Members | Events 入口（計數徽章）；Members 入口（Community 或 Show Members in Channel List，待審核申請）；Server Boosts（管理員） | 未做 | 未排 | |
| 頂端入口：Server Guide 與頻道選擇 | Server Guide（歡迎標牌、待辦、資源頁）、Channels & Roles（onboarding 答案）、Browse Channels（新增/移除頻道） | 未做 | 未排 | status：Browse Channels 未做 |
| 頂端入口：商店與遊戲 | Server Shop（Server Products，US 桌面/網頁實驗）；Game Servers（實驗）；已退役的 Active Now / Server Home（Highlights 動態） | 未做 | 未排 | 實驗功能 |
| Onboarding：加入前後 | 加入前問題（選項指派角色/頻道，單選或多選）；加入後的問題（Channels & Roles）；預設頻道（至少 7 個、5 個以上 @everyone 可寫）；離開後重新加入重走 onboarding | 未做 | 未排 | |
| Server Guide 與歡迎畫面 | Welcome Sign、New Member To-Dos（3–5 項與進度條）、Resource Pages；舊版 Welcome Screen（最多 5 個推薦頻道）；Rules Screening bar（Agree and Finish）；Apply to Join 待審核狀態 | 未做 | 未排 | |
| 頻道右鍵選單：通用 | Mark As Read（Esc）、Invite People / Create Invite、Copy Link（unconfirmed）、Mute Channel（unconfirmed 時長清單）、Notification Settings（All/Only @mentions/Nothing 與沿用預設）、Copy Channel ID（Developer Mode） | 未做 | 未排 | status：頻道右鍵選單 missing |
| 頻道右鍵選單：管理 | Edit Channel、Duplicate Channel、Create Text/Voice Channel、Delete Channel（確認、稽核紀錄原因）、Add to / Remove from Favorites（Nitro 實驗）、Pin channel（2026） | 未做 | 未排 | |
| 頻道右鍵選單：語音 | Set a Channel Status、Open Chat、Hide Names（unconfirmed） | 未做 | 未排 | |
| 建立頻道 | Create Channel 對話框：類型（Text、Voice、Forum、Announcement、Stage、Media）、名稱、Private 開關 | 未做 | 未排 | |
| Edit Channel：概覽 | 名稱（僅 Unicode emoji）與主題（1024；論壇/媒體 4096）；Slowmode（關、5/10/15/30 秒、1/2/5/10/15/30 分、1/2/6 小時）；Content Visibility（年齡限制、劇透頻道，互斥）；Announcement Channel 開關；預設自動封存（1h/24h/3d/1w）；Channel Summaries 開關 | 未做 | 未排 | |
| Edit Channel：語音 | 位元率（96/128/256/384 kbps 依 Boost）、視訊品質（Auto/Full 720p）、使用者上限、區域覆寫 | 未做 | 未排 | 使用者上限與區域覆寫標籤為 unconfirmed |
| Edit Channel：論壇與媒體 | 指南、標籤、預設反應、排序、版面、貼文 slowmode、預設討論串 slowmode、需要標籤；媒體隱藏下載選項 | 未做 | 未排 | 細節見 Threads and forums |
| Edit Channel：權限、邀請、整合 | Permissions 分頁（成員/角色、Deny/Pass-through/Allow、Sync Now、Private）；Invites 分頁（unconfirmed）；Integrations（頻道 webhook、已追蹤頻道）；Delete Channel | 未做 | 未排 | |
| 通知設定（伺服器/分類/頻道） | 伺服器等級（All/Only @mentions/Nothing）；Suppress @everyone 與 @here；Suppress All Role @mentions；Suppress Highlights；Mute New Events；行動推播（超過 2,500 人禁全訊息）；頻道/分類覆寫與靜音 | 部分 | 未排 | 靜音/未讀設定可從 USER_GUILD_SETTINGS_UPDATE 讀取並淡化，無編輯介面 |
| 頻道標頭：名稱與按鈕 | 名稱與主題（點擊看詳情選單）、Threads 按鈕、通知鈴、Pinned Messages popout（Ctrl/Cmd+P）、成員列表開關（Ctrl/Cmd+U）、Search、Inbox（Ctrl/Cmd+I）、Help（Ctrl/Cmd+Shift+H） | 部分 | C（pins popout） | 標頭與成員列表開關存在；Pinned Messages 為 stub，C 切片提供；搜尋欄無結果、Inbox 見 I |
| 頻道標頭：導覽與實驗 | 返回/前進按鈕與滑鼠鍵（2026）；Conversation Summaries 實驗；成員列表 Recent Activity 卡（beta，Hide Activity Cards） | 未做 | 未排 | |

## DMs and friends
| 功能群組 | 官方行為摘要 | velella 狀態 | 排程 | 備註 |
|---|---|---|---|---|
| Home 與 DM 側欄入口 | 側欄頂端條目與 DM 列表；Friends 入口（Online/All/Pending/Blocked/Add Friend）；Nitro 入口（含 Nitro Home）；Shop 入口；Message Requests（有待處理時才出現）；Family Center 分頁（可關閉） | 部分 | 未排 | 只有 Friends 列（顯示 pending 數，點擊無動作） |
| DM 列表列 | 每個 DM/群組一列：頭像、名稱（好友暱稱優先）、presence 點（綠/黃/紅/紫，離線無點）、未讀紅色徽章；DM 預覽時間戳；Compact mode DM 列表 | 完成 | 未排 | 最新優先排序；暱稱與時間戳預覽狀態 status 未提；活動行（DM 下的活動）未做 |
| DM 列表裝飾 | 銘牌（hover/選取時顯示，2025）、DM 列的伺服器標籤（2025）、顯示名稱樣式（2025，僅自訂字型）、被 Ignore 使用者的 DM 淡化與圖示 | 未做 | 未排 | |
| DM 列表管理 | 關閉 DM（hover x，unconfirmed）；Pin DM / 群組（2026）；Favorite DM 與 Show Favorites Server 開關（2026）；伺服器列上方的未讀 DM 頭像泡泡（unconfirmed） | 未做 | 未排 | |
| 建立 DM 與群組 | 「+」/ New Group DM 開啟 Create DM 視窗（好友選擇與搜尋）；Quick Switcher 找人；加好友後的 wave 提示 | 未做 | 未排 | status：create DM / group DM 未做 |
| DM 右鍵選單：個人檔案與社交 | Profile、Add Friend Nickname、Add Note（unconfirmed，500 字、1,500 則）、Invite to Server、Add Friend、Remove Friend（unconfirmed）、Ignore/Unignore、Block/Unblock、Copy User ID（Developer Mode）、Apps 子選單 | 未做 | 未排 | |
| DM 右鍵選單：通知與管理 | Call / Video Call（unconfirmed）、Mute DM / Mute @user（unconfirmed）、Mark As Read（unconfirmed）、Favorite、Pin/Unpin DM、Close DM / Leave Group（unconfirmed 標籤） | 未做 | 未排 | |
| DM 標頭 | 語音/視訊通話、Pinned Messages、Add Friends to DM、使用者個人檔案按鈕；「AKA」提示列出其他名稱；對方的個人檔案側欄（含 Wishlist、共同好友/伺服器） | 部分 | 未排 | 通話按鈕為 stub（status），標頭其餘未做 |
| DM 內的互動 | 待處理好友請求的 Accept Friend Request 按鈕；DM 通話系統訊息（started a call，可由搜尋跳轉）；重啟後重新加入通話（2026）；DM/群組文字聊天內啟動 Activities（2024）；轉傳訊息到 DM（含 @silent） | 未做 | 未排 | 轉傳為 D；通話系統訊息見 Message display |
| Friends：分頁 | Online、All、Pending（接受/拒絕/取消）、View spam requests、Blocked、Add Friend（送出好友請求，錯誤提示） | 唯讀 | 未排 | 只有 pending 計數（RELATIONSHIP_ADD/REMOVE），沒有頁面 |
| Friends：輔助區 | Friend Suggestions、Active Now 欄（好友活動/語音）、好友列 hover 動作（Message、More 選單）、Friends 頁的 New Group DM | 未做 | 未排 | |
| Friends：限制與提示 | 好友上限 1,000，待處理 1,500 收/1,000 發；接受無共同好友/伺服器使用者前的警告；好友請求附註（2026，上限 120 字，unconfirmed） | 未做 | 未排 | |
| Friends：通知與延伸 | 好友上線通知頻率設定；友誼週年提示（unconfirmed）；Game Friends（2025）；Find Your Friends 通訊錄同步（行動版，unconfirmed）；QR code 加好友（僅行動版）；Nitro 好友試用（最多 3 位、2 週）；Riot 好友清單連結（2026） | 未做 | 未排 | QR 加好友、通訊錄同步為行動版專屬 |
| 好友請求與 DM 隱私設定 | Friend Requests（Everyone / Friends of Friends / Server Members）；伺服器成員的私訊開關與套用到全部的提示；Message Requests 開關（成年人，也可 per-server）；DM 垃圾訊息過濾（預設 filter all）；設定改名（Content & Social 變 Messaging Permissions、Activity 變 Games & Apps） | 未做 | 未排 | 見 User Settings |
| 好友暱稱與備註 | 暱稱優先順序（好友暱稱 > 伺服器暱稱 > 顯示名稱，unconfirmed）；群組 DM 以暱稱搜尋；使用者備註（私人、500 字、1,500 則） | 未做 | 未排 | |
| 群組 DM：建立與成員 | 從好友建立（上限 10 人）；1:1 加好友轉為新群組；任一成員可加人；Remove From Group（僅建立者）；Leave Group 與確認；靜默離開（unconfirmed）；轉移擁有者（unconfirmed）；擁有者皇冠（unconfirmed） | 未做 | 未排 | 群組 DM 名稱/圖示的顯示已完成，見下一列 |
| 群組 DM：外觀 | 重新命名（100 字，2026）、自訂圖示（預設為成員頭像拼貼） | 唯讀 | 未排 | 只能顯示名稱與圖示，不能編輯 |
| 群組 DM：邀請 | 邀請連結（桌面任一成員建立，48 小時過期）；擁有者從右鍵 Instant Invite 管理與撤銷；伺服器數量上限時接受不再顯示升級推銷 | 未做 | 未排 | |
| 群組 DM：其他功能 | Clean Up Inactive Group DMs（2026）；群組語音/視訊通話；群組成員列表；群組 pins（任一參與者可釘選）；群組靜音；群組內有被封鎖使用者的一次性警告；遊戲 overlay 建立群組 DM（2026 修正） | 未做 | C（群組 pins） | 遊戲 overlay 見備註；macOS 無 overlay，故該子項 macOS 無此功能；群組 pins 隨 C 的釘選功能 |
| Message Requests | 非好友 DM 導入此處（共同好友或小型伺服器除外，預設開，unconfirmed）；接受/忽略（檢視個人檔案）；回覆自動接受；預覽限制（只有文字、圖片檔案隱藏、連結不可點）；Spam 分頁（接受、舉報垃圾、預覽、忽略）；青少年首次 DM 安全提示 | 未做 | 未排 | |
| 封鎖效果 | 雙向無 DM、好友關係移除、無法送請求、訊息隱藏；被封鎖訊息折疊成可展開列（unconfirmed）；被封鎖/忽略者個人檔案警告；成員列表選取 > 省略號 > Block 與確認；Game Invite 提示不顯示 | 未做 | 未排 | status：blocked users 未做 |
| 忽略效果 | 隱藏新訊息（Show 按鈕展開）、無通知、DM 淡化、舉報流程「More you can do」提供 Ignore | 未做 | 未排 | |
| 已封鎖與已忽略清單 | User Settings > Messaging Permissions 的「Accounts you've blocked」清單 | 未做 | 未排 | |
| 禮物 | 輸入列旁 Gift 按鈕（Nitro/Shop 項目；DM 內完整客製）；從好友 Wishlist 送禮（已送項目自動勾選）；Gift Inventory（找、撤銷、重新產生未領取連結） | 未做 | 未排 | |
| 系統 DM 與 Clyde | Discord Official Messages（來自 Discord Updates、OFFICIAL 徽章、橫幅取代輸入框）；系統 DM 內 App 回應標 APP；Clyde AI 已移除（日期 unconfirmed）；Clyde 系統機器人仍發本機錯誤 | 未做 | 未排 | Clyde 為 unconfirmed |
| Family Center | 青少年與家長的選擇性連結；行動版 QR 連線；家長儀表板（新好友、伺服器等） | 未做 | 未排 | 偏行動版 |

## Message display
| 功能群組 | 官方行為摘要 | velella 狀態 | 排程 | 備註 |
|---|---|---|---|---|
| 訊息類型：一般與回覆 | DEFAULT（0）一般訊息（內容、embed、附件、貼圖）；REPLY（19）上方帶回覆預覽 | 完成 | 未排 | 目前所有訊息都當作一般訊息處理（status），回覆預覽見下方群組 |
| 系統訊息：群組與討論串 | RECIPIENT_ADD/REMOVE（1、2）、CHANNEL_NAME_CHANGE（4）、CHANNEL_ICON_CHANGE（5）、THREAD_CREATED（18）、THREAD_STARTER_MESSAGE（21，unconfirmed）、CALL（3，含 Join 按鈕） | 未做 | 未排 | status：沒有系統訊息 |
| 系統訊息：釘選 | CHANNEL_PINNED_MESSAGE（6）「pinned a message to this channel. See all pinned messages.」 | 未做 | C | C 切片含「pinned a message」系統列 |
| 系統訊息：伺服器事件 | USER_JOIN（7，13 個樣板，unconfirmed）、GUILD_BOOST 與 TIER_1/2/3（8–11）、CHANNEL_FOLLOW_ADD（12）、Discovery 資格通知（14–17）、GUILD_INVITE_REMINDER（22） | 未做 | 未排 | |
| 系統訊息：App 與指令 | CHAT_INPUT_COMMAND（20）與 CONTEXT_MENU_COMMAND（23）的「used /command」標頭（兩者 unconfirmed）、INTERACTION_PREMIUM_UPSELL（26）、GUILD_APPLICATION_PREMIUM_SUBSCRIPTION（32） | 未做 | G（互動標頭） | G 處理 interactions 的 thinking/ephemeral 狀態；指令標頭的呈現是否涵蓋待 G 合約確認 |
| 系統訊息：審核與安全 | AUTO_MODERATION_ACTION（24，來自 embeds[0]）、GUILD_INCIDENT_ALERT_MODE_ENABLED/DISABLED（36、37）、GUILD_INCIDENT_REPORT_RAID/FALSE_ALARM（38、39）、REPORT_TO_MOD 系列（58–62，unconfirmed） | 未做 | 未排 | |
| 系統訊息：Stage 與語音 | STAGE_START/END/SPEAKER/TOPIC（27、28、29、31）、STAGE_RAISE_HAND（30，unconfirmed）、VOICE_SESSION（65，unconfirmed）、CHANNEL_LINKED_TO_LOBBY（49，unconfirmed） | 未做 | 未排 | |
| 系統訊息：訂閱與商店 | ROLE_SUBSCRIPTION_PURCHASE（25）、PURCHASE_NOTIFICATION（44）、HD_STREAMING_UPGRADED（55，unconfirmed）、PREMIUM_GROUP_INVITE（64，unconfirmed） | 未做 | 未排 | |
| 系統訊息：Nitro 與禮物 | PREMIUM_REFERRAL（35）、CUSTOM_GIFT（41）、NITRO_NOTIFICATION（48）、GIFTING_PROMPT（50，本機）、GUILD_BOOST_UPSELL（66，本機）（皆 unconfirmed） | 未做 | 未排 | |
| 系統訊息：其他 | POLL_RESULT（46）、GUILD_DEADCHAT_REVIVE_PROMPT（40）、GUILD_GAMING_STATS_PROMPT（42）、CHANGELOG（47）、IN_GAME_MESSAGE_NUX（51）、GUILD_JOIN_REQUEST 系列（52–54）、FRIEND_REQUEST_ACCEPTED（67）、MEDIA_MENTION_MESSAGE（68）；已移除類型（13、33/34、43…） | 未做 | 未排 | 除 POLL_RESULT 外皆 unconfirmed；已移除類型僅列為參考 |
| 版面：Cozy 與 Compact | Cozy（左側頭像與標頭行）、Compact（隱藏頭像，User Settings > Appearance）、UI density 三段（2025）、訊息群組間距滑桿（0–24px，unconfirmed） | 部分 | 未排 | 只有 Cozy 與 7 分鐘群組；Compact mode 未做（status） |
| 作者行：名稱與顏色 | 名稱優先序（伺服器暱稱、顯示名稱、使用者名稱）；角色顏色選項（Accessibility）與最高顏色角色上色（含強化角色樣式）；Display Name Styles（Nitro 字型、特效、顏色） | 部分 | 未排 | 角色顏色完成；Nitro 顯示名稱樣式未做（status） |
| 作者行：徽章與標籤 | 角色圖示（Boost Level 2）、Server Tag、APP 標籤（原 BOT，unconfirmed）、OFFICIAL / SYSTEM 標籤、OP 標籤（論壇貼文）、跨頻道發布的伺服器標籤（unconfirmed）、頭像裝飾（chat 內 unconfirmed）、Timeout 圖示、Emoji Confetti 徽章 | 未做 | 未排 | |
| 時間戳與編輯標記 | 標頭相對時間（Today at…、Yesterday at…）、hover 完整時間、(edited) 標記與 tooltip | 完成 | 未排 | 短/標頭/tooltip/相對時間格式完成；標記細節為 unconfirmed |
| 提及高亮與點擊作者 | 提及你的訊息整列高亮（user、@everyone/@here、你的角色）；點擊名稱/頭像開個人檔案 popout、右鍵開使用者選單、Shift 點擊 | 部分 | 未排 | 高亮 status 未提；點擊 popout 與使用者選單未做；頭像含 presence |
| 傳送狀態 | 傳送中暗淡、失敗紅字含 Retry 與 Delete | 完成 | 未排 | local echo pending/failed 完成；Retry/Delete 選項是否齊全未列，兩者皆 unconfirmed |
| 隱藏與標記的作者訊息 | 被停權使用者的訊息可隱藏（flag HIDDEN_SUSPENDED_USER）；被標為垃圾的訊息折疊並可從搜尋跳轉；In-Channel Conversation Summaries（實驗） | 未做 | 未排 | |
| 回覆預覽 | 上方曲線連接線、被回覆者小頭像與名稱、內容截斷；點擊跳轉；只有附件時顯示「Click to see attachment」 | 部分 | 未排 | 一層回覆預覽完成；跳轉與附件預覽為 unconfirmed 且 status 未提 |
| 回覆預覽的特殊情況 | 原訊息已刪除（unconfirmed）、被封鎖或被忽略使用者的回覆預覽（unconfirmed） | 未做 | 未排 | |
| 轉傳訊息 | 「Forwarded」標籤與箭頭、快照內容（文字、附件、embed）；轉傳媒體的右鍵選單（Stats for Nerds 等） | 未做 | D | D 含 message_snapshots 渲染；媒體選單另計 |
| 討論串與跨頻道發布呈現 | 討論串起始訊息下方的討論串卡片；跨頻道發布訊息（編輯同步、刪除不同步） | 未做 | 未排 | 討論串卡片依賴討論串視圖（status：thread 檢視未做） |
| 互動回應狀態 | 暫時訊息（Only you can see this・Dismiss，unconfirmed）；互動標頭「{user} used /{command}」（unconfirmed）；延遲的「thinking」載入狀態（LOADING 旗標） | 未做 | G | |
| 基本行內格式 | 斜體、粗體、粗斜體、底線、刪除線、行內程式碼（含連結內行內程式碼顏色，2026）、跳脫字元與跳脫提及 | 完成 | 未排 | |
| 區塊格式 | 標題（# ## ###）、Subtext（-#）、列表（含巢狀）、區塊引用（> 與 >>>）、程式碼區塊（語言標籤語法高亮；unconfirmed 細節） | 完成 | 未排 | 程式碼區塊含 Dark+ 高亮與 Copy |
| 劇透 | 雙豎線劇透，點擊顯示 | 完成 | 未排 | 劇透與選取行為在 selection.md 處理 |
| 連結 | 遮罩連結 [text](url)（離開 Discord 確認與信任網域，unconfirmed）、自動連結、`<url>` 抑制 embed、email 連結 `<mailto>`（2025） | 部分 | 未排 | 連結確認完成；email 連結未提，歸入未完成 |
| 大表情與特殊轉換 | Jumbo emoji（Wumboji，純 emoji 訊息最多 27 個，Compact 不適用）；emoticon 轉換（unconfirmed）；`/me` 斜體（unconfirmed）；不支援語法（表格、圖片、水平線，unconfirmed） | 未做 | 未排 | |
| 提及類標記 | 使用者提及 `<@id>`、角色提及（角色顏色、漸層）、@everyone/@here、頻道提及（類型圖示，unconfirmed）；自我提及不加編輯器高亮 | 完成 | 未排 | 點擊開 popout 未做（見提及高亮群組） |
| 指令與導覽標記 | Slash command mention `</name:id>`（點擊填入輸入框）；Game mention `<@$id>`；Guild navigation（`<id:customize>`、`<id:browse>` 等） | 部分 | 未排 | 解析完成、點擊無動作（status） |
| 表情符號 | 自訂表情（靜態/動態、hover 顯示 `:name:`、點擊 popout，unconfirmed）；Unicode emoji 以 Twemoji 渲染 | 部分 | P1 | 自訂 emoji 完成；Unicode 目前未使用 Twemoji，D4 決議由 P1 帶入 |
| 時間戳標記 | `<t:unix:style>` 九種樣式（t、T、d、D、f、F、s、S、R 相對即時更新） | 完成 | 未排 | |
| 訊息連結 | discord.com/channels/{guild}/{channel}/{message}（@me 用於 DM，unconfirmed），點擊跳轉 | 未做 | 未排 | 只有複製連結，status 未提開啟連結跳轉 |
| Embed：rich 與連結卡 | Rich embed（色條、作者、標題、描述、欄位、縮圖、圖片、頁尾）；article/link Open Graph 卡；多圖 embed 合併成圖庫（unconfirmed）；image embed | 完成 | 未排 | 最多 10 個 embed；多圖圖庫為 unconfirmed |
| Embed：影音 | video embed（YouTube、Twitch 原地播放、垂直 Shorts）、GIFV（GIPHY、Tenor/Klipy 循環靜音）、Spotify 播放器、TikTok 播放（2024）、Tumblr（2025）、AV1（2025） | 部分 | 未排 | 影片 embed 只有預覽圖（status），無播放；GIFV 與其他站台播放未做 |
| Embed：Discord 內容 | poll_result、AutoMod（unconfirmed）、safety（unconfirmed）、gift（unconfirmed）、post_preview（unconfirmed）、邀請卡（unconfirmed）、活動邀請卡（unconfirmed）、遊戲/Rich Presence 邀請、Request to Stream | 未做 | 未排 | |
| Embed 控制 | Suppressed embeds（SUPPRESS_EMBEDS，作者或 Manage Messages 可移除，unconfirmed）；Embed 顯示設定（Text & Images）；Embed Links 權限 | 部分 | 未排 | SUPPRESS_EMBEDS 旗標已遵守（status），移除 embed 的操作、顯示設定未做 |
| 圖片附件 | 內嵌預覽依大小調整，多圖/影片/GIF 拼貼；Alt text（ALT 標記）；Animated 附件（GIF 自動播放、視窗失焦暫停） | 部分 | 未排 | 圖片附件顯示完成；馬賽克拼貼、ALT 標記、動畫控制 status 未提 |
| 媒體檢視器 | 全視窗 lightbox：輪播、縮放、Open in Browser（unconfirmed）；縮圖雜湊預留圖（unconfirmed） | 未做 | 未排 | |
| 影音附件 | 影片內嵌播放器（播放、跳轉、音量、全螢幕、下載）、音訊內嵌播放器（unconfirmed）、語音訊息播放器（波形最多 256 取樣、時長，unconfirmed）、Clips 附件（IS_CLIP） | 未做 | 未排 | 語音訊息只能播放，錄製僅行動版；Clips 的錄製為 Windows-only，但顯示他人 Clip 附件仍可行，故不歸類 macOS 無此功能 |
| 檔案附件 | 純文字檔內嵌預覽（Expand 最多 100 行）、一般檔案卡（圖示、名稱、大小、下載）、附件顯示設定、CDN 連結簽章過期後更新（unconfirmed） | 部分 | 未排 | 只顯示檔名，無下載（status） |
| 附件遮蔽 | 劇透附件（IS_SPOILER，模糊，`SPOILER_` 前綴 unconfirmed）；敏感媒體模糊（依情境 Show/Blur/Block） | 部分 | 未排 | 圖片劇透完成；敏感媒體模糊未做 |
| 貼圖 | PNG、APNG、Lottie、GIF 渲染（最大 320x320，動畫最長 5 秒）；動畫設定（Always/On interaction/Never）；貼圖 popout（unconfirmed）；貼圖 alt text | 未做 | 未排 | status：沒有貼圖 |
| 元件：容器與按鈕 | Action Row（1）、Button（2，Primary/Secondary/Success/Danger/Link）、String Select（3，最多 25 選項）、User/Role/Mentionable/Channel Select（5–8） | 未做 | G | G 為 Apps 與互動流程；元件渲染是否含在內待合約 |
| 元件：V2 版面 | Section（9）、Text Display（10）、Thumbnail（11）、Media Gallery（12）、File（13）、Separator（14）、Container（17）、IS_COMPONENTS_V2 訊息（1<<15，最多 40 個元件） | 未做 | 未排 | |
| 元件：回應 | Modals（標題 45 字、1–5 元件）、Autocomplete choices（最多 25）、Launch Activity、Premium required（已棄用）、Checkpoint card（unconfirmed） | 未做 | G | Modals 在 G 範圍 |
| 投票顯示 | 投票卡（問題 300 字、最多 10 個答案）；投票（選取後 Vote、Remove Vote）；結果顯示（百分比條與計數，unconfirmed）；投票者列表（unconfirmed）；提前結束（unconfirmed）；結束與 POLL_RESULT；限制（不可編輯與轉傳） | 未做 | 未排 | status：沒有投票；MESSAGE_POLL_VOTE_* 未處理 |
| 訊息列表：起點與分隔 | 頻道起點標頭「Welcome to #channel!」、DM 起點標頭（unconfirmed）、日期分隔線 | 部分 | 未排 | 日期分隔線完成；起點標頭 status 未提 |
| 訊息列表：未讀與跳轉 | 未讀紅色「NEW」分隔線、頂部「N new messages」橫幅（Mark As Read、Esc）、Jump to Present 列、跳至訊息的高亮 | 未做 | E | E 含未讀分隔線與新訊息橫幅；Jump to Present 與跳轉高亮 status 為 missing，未排 |
| 訊息列表：捲動與載入 | 雙向無限捲動與骨架載入 | 部分 | 未排 | 向上載入歷史與骨架完成，向下載入未提 |
| 訊息列表：封鎖與忽略 | 被封鎖訊息折疊為「N Blocked Messages — Show」（unconfirmed）；被忽略使用者訊息隱藏於「Show」、DM 變暗 | 未做 | 未排 | |
| 訊息列表：輸入與頻道狀態 | 輸入中提示（{name} is typing… 等）；慢速模式提示與倒數；語音聊天預覽；失去頻道存取時移出（2026） | 部分 | 未排 | 輸入中提示完成；其餘未做 |

## Message actions
| 功能群組 | 官方行為摘要 | velella 狀態 | 排程 | 備註 |
|---|---|---|---|---|
| Hover 工具列：反應 | 建議反應（最常用前 3 個，2024）、Add Reaction 笑臉開啟 emoji 選擇器（選擇器內 Shift 點擊可連加）、Add Super Reaction（Nitro，hover 上的入口為 unconfirmed） | 未做 | J, A | J 建 hover 工具列骨架，A 做反應寫入，P1 做選擇器 |
| Hover 工具列：訊息動作 | Reply、Forward（2024）、Create Thread（需 Create Public Threads，伺服器文字頻道）、Edit（自己的訊息）、More（…，等同右鍵選單） | 未做 | J, B, D | Create Thread 未列入 menu 計畫，屬 未排；Reply 在右鍵選單已有，hover 列沒有 |
| Hover 工具列：其他 | Add Emoji Confetti（實驗，桌面）、圖片右上角的 Apps（App Launcher 圖像編輯） | 未做 | 未排 | 實驗功能 |
| Shift hover 工具列與跳過確認 | 按住 Shift 顯示免確認的即時動作（Delete 等，unconfirmed）；Shift+刪除/取消釘選跳過確認（unpin 已確認）；Option+點擊訊息標為未讀（macOS，unconfirmed） | 未做 | J | J 含 Shift hover 工具列；Shift 跳過刪除確認在 B |
| 右鍵選單：快速反應與反應 | Quick React 列、Add Reaction ›（需 Add Reactions 權限，已有的 emoji 例外）、View Reactions（每個 emoji 與 super 一個分頁） | 未做 | J, A | |
| 右鍵選單：編輯與回覆 | Edit Message（僅自己的訊息，行內編輯）、Reply | 部分 | B | Reply 完成；Edit 於 B |
| 右鍵選單：轉傳 | Forward To 對話框（最多 5 個目的地、搜尋、可附註） | 未做 | D | |
| 右鍵選單：Create Thread | 開啟建立討論串面板（名稱、私人開關，unconfirmed） | 未做 | 未排 | |
| 右鍵選單：複製 | Copy Text（內容，unconfirmed 標籤；Cmd+C）、Copy Message Link、Copy Message ID（Developer Mode）；連結項 Copy Link / Open Link | 完成 | 未排 | C1 選單已有；Copy Message ID 是否僅 Developer Mode 顯示，待 M0 比對 |
| 右鍵選單：Apps | Apps ›：伺服器安裝與使用者安裝的 App 訊息指令 | 未做 | G | |
| 右鍵選單：標為未讀 | Mark Unread（Alt+Enter）：在此訊息前設定讀取標記 | 未做 | E | |
| 右鍵選單：釘選 | Pin / Unpin Message（確認「Oh yeah. Pin it」；DM 中不需權限，伺服器需 Pin Messages） | 未做 | C | |
| 右鍵選單：書籤與提醒 | Bookmark Message（Nitro 實驗，Inbox > Bookmarks，上限 200）、Create Reminder / Remind Me ›（預設延遲或自訂日期時間） | 未做 | I | 依 M0 確認帳號是否有此實驗 |
| 右鍵選單：Speak Message | 以系統 TTS 朗讀訊息（unconfirmed，官方文件未載） | 未做 | J | |
| 右鍵選單：頻道類功能 | Publish（公告頻道，推送給追隨的伺服器，自己的訊息需 Send Messages） | 未做 | 未排 | |
| 右鍵選單：管理 | Remove Embed(s) / Suppress Embeds（作者或 Manage Messages，標籤 unconfirmed）；Remove Reactions › / Remove All Reactions（Manage Messages，確認）；End Poll（unconfirmed）；Mark as Not Sensitive（模糊媒體） | 未做 | A（移除反應） | Suppress Embeds、End Poll、Mark as Not Sensitive 未排 |
| 右鍵選單：媒體與選取 | 圖片：Copy Image、Save Image、Copy Link、Open Link（unconfirmed）；選取文字：Copy 與 Search with {Google/Bing/DuckDuckGo/自訂} | 部分 | J | Copy（選取）與 Open Link 完成；圖片項由 J；「Search with Google」在 selection.md 為非目標，未排 |
| 右鍵選單：回報與刪除 | Report Message（選類別與原因，再「More you can do」）、Delete Message（自己的或 Manage Messages，含預覽的確認） | 未做 | H, B | Report 在 H；Delete 在 B |
| 右鍵選單鍵盤開啟 | Shift+F10 / 選單鍵開啟（含輸入框內，2026） | 未做 | 未排 | 選單內鍵盤導覽已完成 |
| 訊息焦點與導覽鍵 | Tab 進入訊息列表、方向鍵在訊息間移動、Tab 進入訊息；F6 / Shift+F6 跳區塊；Page Up/Down 捲動；Shift+Page Up 跳到最舊未讀；Esc / Shift+Esc 標記頻道/伺服器已讀 | 未做 | 未排 | 標記已讀與 E 的 ack 有關，但鍵盤綁定本身未排 |
| 訊息單鍵動作 | Q 引用、+ 加反應、R 回覆、E 編輯（自己）、Backspace 刪除、P 釘選、Cmd+C 複製內容、Alt/Option+Enter 標為未讀、F 轉傳（Discord blog，unconfirmed） | 未做 | 未排 | M0 會記錄官方鍵盤清單；綁定尚無切片 |

## Composer
| 功能群組 | 官方行為摘要 | velella 狀態 | 排程 | 備註 |
|---|---|---|---|---|
| 輸入框基礎 | 富文字輸入框（即時 markdown 預覽：劇透底色、程式碼區塊偵測）；占位文字「Message #channel / @user」（unconfirmed）；Enter 送出與 Shift+Enter 換行；拼字檢查；Home/End 鍵（2026） | 部分 | 未排 | 純文字欄位、Return 送出完成；其餘未做 |
| 草稿 | 切換頻道時保留未送文字（unconfirmed）；討論串另計 | 完成 | 未排 | 每頻道草稿 |
| 字數限制 | 2000 字（Nitro 4000），接近上限出現計數器 | 未做 | 未排 | |
| 格式快捷鍵與劇透按鈕 | Cmd+B/I/U、選取文字時的浮動格式工具列（unconfirmed）；選取後點眼睛圖示變劇透 | 未做 | 未排 | |
| 自動完成：提及與頻道 | @ 提及（成員、角色、@everyone/@here、@time、@game，2026）；# 頻道（含討論串，unconfirmed） | 未做 | 未排 | |
| 自動完成：emoji 與貼圖 | : 觸發 emoji 建議（依使用頻率、膚色）、輸入時的貼圖建議（Up 進入、左右選擇、Enter） | 未做 | 未排 | emoji 自動完成與 P1 的選擇器不同 |
| 斜線指令 | / 指令選擇器（內建與 App 指令、App 圖示列、frecency、選項 chip）；內建指令 /tts、/spoiler、/me、/shrug、/tableflip、/unflip、/nick…（unconfirmed） | 未做 | 未排 | 互動送出在 G 之後才有基礎，但輸入端未排 |
| @silent | 前綴 @silent 送出不推播（SUPPRESS_NOTIFICATIONS 旗標） | 未做 | 未排 | |
| 「+」選單與上傳 | 「+」選單（Upload a File、Create Thread、Create Poll、Use Apps/Activities，unconfirmed）；上傳（按鈕、拖放、貼上、Cmd+Shift+U，unconfirmed）；附件托盤（Modify：改名、alt text、劇透、移除）；明確媒體上傳阻擋 | 未做 | 未排 | 「+」選單是 stub，拖放為 CLAUDE.md 所列入口 |
| 選擇器按鈕 | Emoji 選擇器（分類、搜尋、伺服器區段、Nitro 鎖定，unconfirmed）；GIF 選擇器（2026 供應商由 Tenor 換為 Klipy/GIPHY，unconfirmed）；貼圖選擇器（標準包、伺服器貼圖、Nitro 跨伺服器） | 未做 | P1（emoji 選擇器） | GIF/貼圖/emoji 按鈕目前為裝飾；P1 的選擇器是獨立視圖，是否接入輸入框待 P1 合約 |
| 禮物、Apps 與 Confetti 按鈕 | Gift 按鈕（Nitro/Shop 禮物）；App Launcher 按鈕（Recents、Collections、搜尋、App 指令）；Emoji Confetti 按鈕與橫幅 | 未做 | 未排 | |
| 投票建立 | 問題、最多 10 個答案（可附 emoji）、持續時間（1h、4h、8h…）、多選 | 未做 | 未排 | |
| 語音訊息錄製 | 僅行動版（按住麥克風、滑動鎖定、垃圾桶取消）；桌面只能播放 | macOS 無此功能 | 未排 | 桌面官方客戶端也無錄製；播放見 Message display |
| 回覆模式 | 輸入框上方「Replying to {name}」與 @ ON/OFF 提及開關 | 部分 | 未排 | 回覆列與取消完成；@ ON/OFF 開關未提 |
| 編輯模式 | 行內編輯器取代訊息，Enter 儲存、Esc 取消（unconfirmed 細節）；空輸入框按 Up 編輯上一則自己的訊息 | 未做 | B | |
| 輸入中事件送出 | 打字時送出 typing 事件（約每 10 秒，unconfirmed） | 未做 | 未排 | status：send typing 未做 |
| 輸入框狀態：發送限制 | Slowmode 倒數（5 秒–6 小時，Bypass Slowmode 權限）、Timed-out 狀態（輸入框停用）、無權限狀態（unconfirmed 文案）、DM 被封鎖或隱私設定禁止（unconfirmed） | 未做 | 未排 | F1 的權限計算是前置基礎但輸入框狀態未排 |
| 輸入框狀態：頻道與伺服器 | 鎖定/封存討論串（封存討論串在有權限時傳送會解封）、伺服器預覽（lurker）顯示 Join Server、驗證等級閘門、成員審核規則「Complete」（皆 unconfirmed） | 未做 | 未排 | |
| 論壇輸入框 | Ctrl/Cmd+Enter 在論壇輸入框發文（unconfirmed） | 未做 | 未排 | |
| 遊戲 overlay 輸入框 | 遊戲內 overlay 的文字輸入 | macOS 無此功能 | 未排 | Overlay 為 Windows 專屬 |
| 官方未提供的功能 | 排程訊息（官方無，unconfirmed）；訊息翻譯（官方無原生翻譯） | macOS 無此功能 | 未排 | 官方客戶端本身沒有此功能（2026 年 10 月），僅記錄 |

## Reactions
| 功能群組 | 官方行為摘要 | velella 狀態 | 排程 | 備註 |
|---|---|---|---|---|
| 反應顯示 | 訊息下方 emoji 加計數的 pill，自己反應過的藍紫色框；pill 後的新增反應按鈕；上限 20 種（錯誤 30010） | 唯讀 | A | 顯示完成（上限 20 顯示），點擊切換於 A |
| 反應權限與切換 | 只有新 emoji 需要 Add Reactions，另需 Read Message History；點擊 pill 切換 | 未做 | A | |
| Super Reactions | 選擇器內切換 Super Reaction 圖示送出動畫反應（burst）；動畫開關（Accessibility > Play animated emoji，含 Reduced Motion） | 未做 | A | |
| 多重反應與快速反應 | 選擇器內 Shift 點擊連加；Quick React 與建議（前 3 個，2024）；反應 frecency（unconfirmed） | 未做 | A, P1 | frecency 由 D3 讀寫設定 proto |
| View Reactions 與移除 | 每個 emoji（含 super）一個分頁列出使用者，X 移除（需 Manage Messages）；Remove all / remove emoji（unconfirmed） | 未做 | A | |
| 反應通知與設定 | 實驗：Settings > Notifications > Reaction Notifications；Show reactions 設定（render_reactions，unconfirmed） | 未做 | 未排 | |
| 論壇與行動版反應 | 論壇貼文卡的 React 按鈕（2026）；行動版雙擊以常用 emoji 反應 | 未做 | 未排 | 雙擊為行動版專屬，桌面無對應 |

## Search, Inbox & pins
| 功能群組 | 官方行為摘要 | velella 狀態 | 排程 | 備註 |
|---|---|---|---|---|
| 搜尋列與快捷鍵 | 頻道/DM 標頭右上角搜尋列（範圍為目前伺服器或 DM）；Ctrl/Cmd+F 對目前頻道預填 in:；Ctrl/Cmd+Shift+F 全頻道；清除（X）按鈕（2026） | 部分 | 未排 | 搜尋欄存在但沒有結果（status） |
| 搜尋篩選器 | 篩選建議 popout；from:、in:（行動版可選所有 DM，2025）、mentions:（含角色，unconfirmed）；has: link/embed/poll/file 與 forward、image/video/sound/sticker（unconfirmed）；負向篩選 -（2026）；authorType: user/bot/webhook（2025）；before:/after:/on 與日期選擇器；during:、pinned:（unconfirmed）；進階篩選（回覆對象、連結網域、附件檔名與副檔名等）；More filters 組合 | 未做 | 未排 | |
| 搜尋結果面板 | 側欄面板含訊息預覽與頻道標頭；排序（Newest/Oldest/Most Relevant）；上限 5,000 筆或 200 頁；結果分頁 Messages/Media/Files/Links（iOS 已確認，桌面 unconfirmed）；劇透頻道媒體保持劇透；系統訊息顯示（2026）；漸層角色顏色（2026）；索引（大型伺服器首次可能延遲） | 未做 | 未排 | |
| 搜尋結果互動 | 點擊跳至訊息；跳至垃圾標記結果（2026）；方向鍵加 Enter 導覽結果（2026）；論壇貼文搜尋（獨立搜尋框含 Clear）；搜尋歷史（unconfirmed） | 未做 | 未排 | |
| DM 與跨 DM 搜尋 | DM/群組 DM 內搜尋該對話；跨所有 DM（行動版，桌面僅限實驗） | 未做 | 未排 | |
| Quick Switcher | Ctrl/Cmd+K 跳到頻道、DM、伺服器並顯示上線狀態；前綴 @ # ! *（unconfirmed）；輸入名稱不帶 @ 的提及建議（桌面實驗） | 未做 | 未排 | status：quick switcher 未做 |
| Inbox 外殼 | 標題列 Inbox 按鈕（2025）、Ctrl/Cmd+I、分頁 For You / Unreads / Mentions / Bookmarks | 未做 | I | I 含 Inbox 與其分頁 |
| Inbox：For You | 個人化動態（伺服器圖示）、AutoMod 警示含 Jump（unconfirmed） | 未做 | I | |
| Inbox：Unreads | 依頻道分組的未讀（來自未靜音頻道與 DM/群組）；View all unreads（2026）；Notifications Disabled 分隔線（2026）；單項標已讀、Mark Inbox As Read、Ctrl/Cmd+Shift+E；內嵌互動（反應、播放媒體、YouTube）；從 Inbox 靜音；Enter 導覽（2026） | 未做 | I | |
| Inbox：Mentions | 提及你的訊息（@username、@role、@everyone）；篩選（Include @everyone、@role…）；X 移除（7 天自動清除）；Jump 按鈕；點擊開個人檔案卡（2026）；舊式提及 popout 快捷鍵 Ctrl+Alt+@ | 未做 | I | |
| 釘選 popout | 標頭釘選圖示與 Ctrl/Cmd+P；列出（新到舊）；Jump；popout 內 X 取消釘選（確認，Shift 點擊跳過）；討論串釘選從 Split View 下拉（2026）；螢幕閱讀器朗讀釘選 | 未做 | C | 目前只有 stub |
| 釘選動作與規則 | Pin Message（hover > More，確認「Oh yeah. Pin it.」並發系統訊息）；Pin Messages 權限（2025）；上限 250（2025，滿時需取消）；DM/群組任一參與者可釘選；右鍵 Unpin Message；「pinned a message」系統訊息（本身不可釘選） | 未做 | C | |
| 書籤 | Bookmark Message（右鍵或訊息上的書籤按鈕）；上限 200；Inbox > Bookmarks 檢視；取消儲存或轉為提醒；來源頻道失去存取時顯示佔位（2026）；僅部分 Nitro 成員的實驗 | 未做 | I | |
| 提醒 | Create Reminder（Inbox 訊息上 > Remind Me 預設或自訂）、Bookmarks 分頁的提醒列表與「Due in」標籤；編輯/完成/移除提醒；到期通知行為（unconfirmed） | 未做 | I | 到期通知如何觸發由 M0 量測 |

## Member list & profiles
| 功能群組 | 官方行為摘要 | velella 狀態 | 排程 | 備註 |
|---|---|---|---|---|
| 成員列表開關與分組 | 標頭圖示或 Ctrl/Cmd+U 開關；hoist 角色各自一區；非 hoist 的上線成員歸 Online、離線歸 Offline；各區標頭顯示「ROLE — N」（unconfirmed） | 完成 | 未排 | 以 op 37 範圍 [[0,99]] 惰性載入；官方快捷鍵 Cmd+U 是否已綁定 status 未提 |
| 成員名稱樣式 | 最高顏色角色上色與「Show role colors next to names」；強化角色樣式（Solid/Gradient/Holographic）；角色圖示；伺服器擁有者皇冠（unconfirmed） | 部分 | 未排 | 角色顏色完成；漸層/全像、角色圖示、皇冠未做 |
| 成員標籤與徽章 | APP 標籤、OP 標籤、已驗證 Bot 勾（unconfirmed）、OFFICIAL 徽章、伺服器標籤（4 字元）、Linked Role 標籤、Timeout 指示 | 未做 | 未排 | |
| 成員列裝飾 | 銘牌（伺服器、頻道、討論串成員列表）；顯示名稱樣式（DM/群組完整，伺服器僅字型）；「Can't Wait For」「Obsessed With」新狀態類型 | 未做 | 未排 | |
| 成員 presence 與狀態行 | 上線/閒置/DND/直播點、離線灰階；自訂狀態覆蓋活動並以省略號截斷 | 完成 | 未排 | |
| 成員活動卡 | Beta 桌面活動卡（遊戲/音樂，收合 3 張或展開）、成員列表最近活動（2024） | 未做 | 未排 | |
| 其他成員列表 | 討論串成員列表；群組 DM 成員列表（可切換、檢視個人檔案/DM、擁有者踢人）；語音訪客（Kick Member，2025）；大型伺服器更快載入（2025）；被踢/封鎖者即時移除（2026）；點擊角色提及顯示該角色成員（2026） | 部分 | 未排 | 只支援伺服器頻道（status：partial），GUILD_MEMBER_LIST_UPDATE 已處理 |
| 伺服器 Members 頁與 Mod View | 桌面限定管理表格（加入日期、帳號年齡、加入方式、角色…）；成員列 Open Mod View（管理工具、帳號資訊、訊息） | 未做 | 未排 | |
| Popout：開啟與外觀 | 點擊名稱/頭像開啟（點名稱顯示伺服器個人檔案）；橫幅（Nitro 680×240，GIF 或顏色）；頭像加裝飾；個人檔案特效（Reduced Motion 停用）；個人檔案外框；主題色（Primary + accent 漸層）；Sync profile themes | 未做 | 未排 | status：個人檔案 popout 未做 |
| Popout：資訊區塊 | 顯示名稱（含樣式）、使用者名稱、代名詞；伺服器標籤（點擊開 Server Profile，Join/Apply）；徽章列（hover tooltip，點擊開 Badge Directory）；自訂狀態泡泡（2024，Send Reaction / Reply to Status）；About Me（300 字，markdown、連結、emoji）；加入時間（unconfirmed 版面）；角色列表（可增減，Manage Roles）；備註欄 | 未做 | 未排 | |
| Popout：活動與連結 | 活動卡堆疊（遊戲、音樂、語音；Join Voice、Ask to Join）；Game Profile（類型、截圖、平台、評論）；連結的帳號（Display on profile）；訊息框與快捷動作（2024）；共同伺服器/好友（私人檔案也顯示）；View Full Profile；View main profile；Copy Avatar Image Link；語音使用者 hover popout；私人檔案（隱藏狀態、代名詞、徽章…） | 未做 | 未排 | |
| 完整個人檔案 | Activity 分頁（即時活動、最近遊戲 2024、清除遊戲歷史）；Mutual Servers 與 Mutual Friends 分頁；Wishlist 分頁（公開/私人、加入項目、送禮 2025）；Profile Board 小工具（Favorite Game、Currently Playing 5 筆、Want to Play 20 筆等）；桌面改版（2025）；Badge Directory（方向鍵導覽，2026） | 未做 | 未排 | |
| 個人檔案編輯：身分 | Profiles 設定頁（User Profile / Per-server Profiles）；頭像（PNG/GIF，8 MB，裁切，最近 6 張）；顯示名稱（1–32）與使用者名稱（2–32，每週可改兩次）；顯示名稱樣式（Nitro 12 字型 7 效果）；Legacy Username 徽章開關；伺服器暱稱（Change Nickname 權限、/nick） | 未做 | 未排 | 見 User Settings |
| 個人檔案編輯：外觀 | 頭像裝飾；銘牌；橫幅（Nitro，<10 MB，裁切）；個人檔案主題；About Me 編輯器（WYSIWYG，300 字）；代名詞；個人檔案特效；外框；小工具編輯；最近遊戲與清除歷史 | 未做 | 未排 | |
| 個人檔案編輯：伺服器與隱私 | 伺服器標籤選擇（一次一個，2025）；Per-server 個人檔案（Nitro：頭像、橫幅、bio、主題、裝飾、特效、銘牌…）；個人檔案隱私（Profile Privacy）；活動隱私（Share Activity with Others 與 per-server）；連線顯示開關；Reduced Motion、Sync profile themes、Display Name Styles 的無障礙開關；Server Profile 伺服器卡 | 未做 | 未排 | |
| 使用者右鍵選單：基本 | Profile、Mention（unconfirmed）、Message（unconfirmed）、Call（unconfirmed）、Add Note（unconfirmed 位置）、Add Friend Nickname、Invite to Server、Add/Remove Friend、Ignore/Block（確認）、Mute（本機靜音其語音，unconfirmed）、Copy User ID | 未做 | 未排 | status：成員右鍵選單未做；Message 的 DM 入口也未做 |
| 使用者右鍵選單：管理 | Edit per-server profile / Change Nickname（自己或有權限者）、Roles 子選單、Apps 子選單、Timeout / Remove Timeout（60s…1w 與原因）、Kick / Ban（原因對話框，標籤 unconfirmed）、Open Mod View、Report（unconfirmed） | 未做 | 未排 | Apps 的訊息指令在 G，但使用者上下文指令未排 |
| 徽章：訂閱與活動 | Nitro（演進式 tenure）、Server Booster（連續演進）、Discord Quests（2024-04-24 後）、Orbs（2025）、Last Meadow Online（2026）、Gifting 徽章（實驗）、Account Age / Streaming / Game Time / Game Variety（實驗） | 未做 | 未排 | |
| 徽章：身分與傳承 | Legacy Username、Bug Hunter / Golden、Discord Staff、HypeSquad Events 與 Houses、Moderator Programs Alumni、Early Supporter、Partnered Server Owner、Early Verified Bot Developer、Active Developer（unconfirmed） | 未做 | 未排 | 多數為傳承徽章 |

## Presence & status
| 功能群組 | 官方行為摘要 | velella 狀態 | 排程 | 備註 |
|---|---|---|---|---|
| 狀態選單 | 點自己頭像（左下）開啟狀態選單（Set Your Status）；Online、Idle（手動）、Do Not Disturb、Invisible；狀態期限（unconfirmed）；跨裝置同步（unconfirmed）；自動 Idle（逾時 unconfirmed）；「Clear Status When I Look Away」（2025）；平台指示（desktop/mobile/web/vr，unconfirmed） | 未做 | 未排 | 自己的頁尾圓點固定為 online（status：partial） |
| 他人 presence 顯示 | 綠/黃/紅/紫圓點、離線無點且名稱灰階、直播紫色、AFK 頻道移動不再切回 Online | 完成 | 未排 | 好友與 DM 對象的 PRESENCE_UPDATE；直播紫色與平台指示 status 未提 |
| 自訂狀態：設定 | 頭像選單 Add Status：文字加 emoji、Save、編輯（鉛筆）；最長 128 字（unconfirmed）；Unicode emoji 免費、自訂伺服器 emoji 需 Nitro；Clear After（預設一天，完整清單 unconfirmed）；同一對話框的 Status 下拉；提示式標籤「Can't Wait For」「Obsessed With」（unconfirmed） | 未做 | 未排 | status：自己的自訂狀態未做 |
| 自訂狀態：顯示與互動 | 取代成員列表與 DM/好友列表的活動行；對他人自訂狀態回應表情或回覆；Hang status（僅語音，unconfirmed）；Voice channel status（語音頻道狀態文字，需權限） | 部分 | 未排 | 他人的自訂狀態行完成；狀態回應、Hang status、語音頻道狀態未做 |
| 活動類型與顯示 | Playing/Streaming/Listening to/Watching/Competing 等活動類型；成員列表顯示 Name/State/Details；Game Profile 頁；活動歷史（30 天、Clear Game History）；多活動卡堆疊 | 未做 | 未排 | status：activities 顯示未做 |
| Rich Presence 欄位 | details/state（含連結）、start/end 時間戳（進度條與 elapsed）、large/small 圖像與 hover 文字、party（id 與 [current, max]）、最多 2 個連結按鈕（1–32 字）、Join/Spectate/Ask to Join（secrets 與旗標）、遊戲更新頻率（20 秒 5 次） | 未做 | 未排 | |
| 遊戲與服務偵測 | PC 遊戲自動偵測、主機遊戲（Xbox/PlayStation）；Spotify 聆聽狀態（Play on Spotify、Listen Along）；Crunchyroll 觀看狀態 | 未做 | 未排 | PC 遊戲偵測在 macOS 的支援範圍 unconfirmed，歸 未做 |
| 活動隱私 | 見 Games & Apps 設定：Share my activity、per-server、join 權限 | 未做 | 未排 | 見 User Settings |

## Notifications
| 功能群組 | 官方行為摘要 | velella 狀態 | 排程 | 備註 |
|---|---|---|---|---|
| 通知設定頁 | User Settings > Notifications（2026 年 3 月重新設計）；桌面通知開關與 DM「Turn on notifications」提示（unconfirmed）；In-app toast 通知開關 | 未做 | 未排 | status：沒有通知，設定視窗未做 |
| 徽章與工作列 | Enable Unread Message Badge（Dock/工作列未讀徽章，unconfirmed）；Enable Taskbar Flashing（Windows，unconfirmed）；Push Notification Inactive Timeout（行動推播，unconfirmed） | 未做 | 未排 | Dock 徽章 macOS 可行但未做；Taskbar Flashing 為 Windows 專屬，已與 Dock 徽章同列，狀態以 Dock 徽章為準 |
| 通知類型開關 | Reaction Notifications（All/Only DM/Never，實驗）、Game Activity（實驗）、Community Activity Alerts（管理員 raid/DM 垃圾訊息警示）、Voice Activity（2026 年 5 月恢復）、Friend online 通知、Friend Gaming Activity / Profile Updates、Highlight、Go Live 通知（串流者勾選，伺服器低於 50…）、其他類型（好友週年、遊戲更新、伺服器熱門、DM 回覆提醒…，unconfirmed） | 未做 | 未排 | |
| 文字轉語音與 Email 通知 | TTS 通知（所有頻道/目前頻道/Never）；Email 通知（Communication、Social、Announcements & Updates、Tips、Recommendations） | 未做 | 未排 | |
| 通知音效開關 | Disable All Notification Sounds、Message/Deafen/Undeafen/Mute/Unmute、User Join/Leave、Stream Started/Stopped、Viewer Join/Leave、Invited to Speak（皆 unconfirmed）；PTT Activate/Deactivate；Incoming Ring（停用連帶關閉事件音效包）；相機開關音效（2025） | 未做 | 未排 | |
| 事件音效與進場音效 | Event Sound Pack（Winter、Halloween 等）；每伺服器自訂通知音（custom_notification_sound_config，unconfirmed）；Entrance Sounds（Nitro，Voice & Video > Soundboard） | 未做 | 未排 | |
| 伺服器通知設定 | 右鍵伺服器 > Notification Settings；Mute server 與時長（15 分、1/3/8/24 小時、直到開啟）；層級 All/Only @mentions/Nothing；Suppress @everyone/@here、All Role @mentions、Highlights；Mute New Events；Mobile Push；Hide Muted Channels | 未做 | 未排 | 靜音狀態可讀並淡化，沒有編輯介面 |
| 頻道與討論串通知 | Notification Overrides（頻道/分類：All/Mentions/Nothing/Mute，Use Category/Server Default）；標頭鈴鐺靜音；討論串通知（All/Mentions/Nothing、加入/離開）；論壇貼文追蹤；Highlights 摘要與個人化開關；Raid alert push（unconfirmed） | 未做 | 未排 | |
| 提及穿透與回覆 | 提及（直接、角色、@everyone/@here）穿透靜音並顯示紅色徽章；回覆預設 ping 作者與 @ ON/OFF；@silent；跳脫的 @everyone/@here 不通知（2026）；未讀/提及徽章（伺服器圖示、頻道、DM，含靜音頻道的提及） | 部分 | 未排 | 徽章完成；回覆 ping 開關與 @silent 未做（見 Composer） |
| Inbox 通知與提醒 | Inbox Mentions 分頁（篩選、移除）與 Unreads 分頁（Mark Inbox As Read、快捷鍵）；其他分頁 For You/Bookmarks/Scheduled/Game Invites（unconfirmed）；書籤與提醒（Remind Me 預設/自訂、Due in） | 未做 | I | 詳細列在 Search, Inbox & pins |
| 其他通知 | 好友請求通知（Pending 分頁徽章與 email）；訊息請求通知；DND 行為（靜默桌面通知）；Quest 通知（Hide This）；Focus / quiet mode（proto quiet_mode，unconfirmed）；行動版獨立設定 | 部分 | 未排 | Friends 列顯示 pending 數；其餘未做；行動版設定在桌面無對應 |

## Voice & video
| 功能群組 | 官方行為摘要 | velella 狀態 | 排程 | 備註 |
|---|---|---|---|---|
| 加入語音與權限 | 點語音頻道加入（鎖頭表示無 Connect）；Connect/Speak/Video 權限；伺服器語音頻道有鏡頭時最多 25 人與「channel full」；Switch to Voice Channel 快捷鍵；斷線（可設定快捷鍵） | 未做 | 未排 | status：voice connect 未做，VOICE_SERVER_UPDATE 未處理 |
| 語音連線狀態與 DAVE | 連線狀態（Connecting、RTC Connecting、ICE Checking、No Route…）；「Connecting」卡死減少約 30%（2026）；沒有 DAVE 的客戶端被語音 gateway 以 4017 拒絕（help 文章 2026-03-02，無獨立生效日） | 未做 | 未排 | velella 必須實作 DAVE 才能加入語音/視訊/Go Live，屬架構級工作 |
| 語音面板與連線資訊 | 左下語音狀態面板按鈕（鏡頭、螢幕分享/Go Live、斷線）；Voice/Video Details（綠色鎖「End-to-end encrypted」）；ping 圖與封包遺失（unconfirmed）；語音區域（自動，首位加入者決定）；DM 通話區域跟著撥號者 | 未做 | 未排 | |
| 語音頻道資訊 | 頻道位元率上限（Level 1–3）；頻道狀態；活躍計時（2025）；收合時的頭像 facepile；使用者 popover（Rich Presence 遊戲、音量）；Hang Status（實驗，unconfirmed） | 部分 | 未排 | 頻道列表有在線者列表，其餘未做；詳見 Channel list |
| 語音內文字與邀請 | Text Chat in Voice（Open Chat 泡泡）；Invite to Voice（建議成員與親近好友）；語音邀請 embed（顯示已在通話者）；Request to Stream（對參與者要求串流）；串流觀眾顯示在頻道列表 | 未做 | 未排 | |
| 語音管理 | 拖曳成員移動（Move Members，含訪客）；Server Mute/Deafen/Disconnect/Move（版主右鍵）；Priority Speaker 權限；Living Room（實驗，極少數伺服器） | 未做 | 未排 | |
| 使用者面板：靜音與拒聽 | 麥克風靜音（頭像右側指示器）、拒聽（同時靜音）；User Settings 齒輪；輸入選項選單（輸入/輸出裝置、Push to Talk、Voice Settings）；裝置類型圖示 | 部分 | 未排 | 齒輪與按鈕為 stub（status：設定視窗未做）；靜音/拒聽未實作 |
| 通話檢視 | Grid View（預設）、Focus View（Hide/Show Members）、Pop Out View（獨立視窗、Stay On Top）、Full Screen View（Text in Voice 可用）、單一磚塊 Pop out、Show Non-Video Participants、磚塊選項（停止觀看串流等）、整合控制列（鏡頭等）、控制列閒置自動隱藏 | 未做 | 未排 | |
| 通話指示與音量 | 說話指示（綠色框/環）；磚塊與列表的靜音/拒聽圖示；伺服器靜音/拒聽專用圖示（unconfirmed）；每人音量（0–200% unconfirmed）；本機靜音某人；語音列表的 Nitro 顯示名稱樣式 | 未做 | 未排 | |
| 通話視窗行為 | 螢幕分享縮放與平移（滾輪/觸控板）；Picture-in-Picture 浮動視窗；觀看影片/串流時螢幕保持不休眠 | 未做 | 未排 | |
| 鏡頭 | 開關鏡頭（語音面板按鈕）；DM 標頭 Start Video Call / Start Voice Call；鏡頭選擇與 Test Video；Always preview video / Preview Camera；開鏡頭音效（2025） | 未做 | 未排 | DM 通話按鈕目前是 stub |
| 視訊背景與編碼 | Background Blur（免費，僅桌面）；四個預設背景；自訂背景（Nitro，JPG/PNG/GIF/MP4，10 MB，每帳號 10 個）；通話中更換背景；視訊濾鏡（官方無文件，unconfirmed）；視訊編碼（OS 編碼器，AVC/H.264 為鏡頭與 Go Live 必要，AV1…）；巴西地區視訊不可用（2026） | 未做 | 未排 | Linux 背景替換（2026）為 Linux 專屬，已併入此列不另計 |
| Go Live：開始分享 | 語音面板螢幕圖示（DM 內 Share Your Screen）；macOS 分享流程（整個螢幕/視窗/App/「Or share a device」，品質設定）；串流遊戲（需偵測，Add it!）；串流者 PiP 預覽；Change Window / Stop Streaming | 未做 | 未排 | macOS 螢幕錄製權限流程待設計 |
| Go Live：品質與音訊 | 解析度與幀率（免費 720p30，Nitro 至 4K60）；串流音訊（Windows 與 macOS 桌面 App、Chrome、行動版）；同時最多 50 人分享影像；Faster Go Live 啟動（單視窗串流） | 未做 | 未排 | |
| Go Live：觀看 | Watch Stream（雙擊串流者）；Multistream（眼睛圖示，僅桌面）；串流音量與靜音；觀眾 Pop Out；觀眾列表（眼睛圖示）；LIVE 徽章；Stream Previews 設定與 Hide stream preview | 未做 | 未排 | |
| Go Live：隱私與邀請 | Stream Privacy Code（E2EE）；Invite to Stream 與串流邀請 embed（Join 按鈕）；串流錯誤碼（2001 啟動失敗、2002–2006 低 fps/封包遺失…） | 未做 | 未排 | |
| DM 與群組通話 | 開始通話（右上電話/鏡頭圖示）；群組上限 10 人（通話持續到無人）；來電視窗（Answer Cmd+Return、Decline Esc）；來電與撥號音可在 Notifications 關閉；行動版撥號確認；逐一撥號給未加入成員（unconfirmed） | 未做 | 未排 | 通話按鈕為 stub；撥號確認為行動版專屬 |
| Stage 頻道（語音端） | 僅 Community，非 E2EE；角色（版主/講者/聽眾，聽眾靜音進入）；Start the Stage（需主題、通知）；主題顯示；Request to Speak；Invite to speak、Show Requests；Allow requests to speak 開關；Move to Audience；Disconnect from Stage；Exit Quietly；等待音樂；Stage 內鏡頭與 Go Live（1 串流、最多 5 鏡頭）；Stage 文字聊天；End Stage；Stage pop-out；Stage 權限（Manage Channels、Mute Members、Move Members） | 未做 | 未排 | 頻道列的 Stage 列已完成；此群組為 Stage 內的實際互動，與 Channel list 的 Stage 流程重疊 |
| 音效板（Soundboard） | 開啟（通話列/面板的彩帶圖示）；Play（全頻道聽到，拒聽者聽不到）；Preview（僅自己）；Favorite；預設音效（unconfirmed）；伺服器音效欄位（8，Boost T1/T2/T3 為 24/36/48，Add Sound）；權限（Use Soundboard、Create/Manage Expressions）；跨伺服器音效需 Nitro；進場音效（Nitro）；Soundboard Volume（跨裝置同步）；加入時預載（2026） | 未做 | 未排 | |
| Activities | App Launcher 按鈕（輸入列右側與通話底部：Recents、Collections…）；語音中啟動 Activity（首次需授權）；加入他人的 Activity；文字頻道/DM 內由「+」啟動（不需語音）；Pop-out Activities；Watch Together 主持控制（2024）；語音中的 Rocket 按鈕（2024） | 未做 | 未排 | |
| 語音設定：輸入輸出 | Input/Output Device、Input/Output Volume、Mic Test（Let's Check）、Input Mode（Voice Activity 或 Push to Talk）、自動判定輸入靈敏度與手動滑桿、Push to Talk 快捷鍵與 Release Delay、PTT 啟用/停用音效 | 未做 | 未排 | PTT 全域快捷鍵需要輔助使用權限 |
| 語音設定：音訊處理 | Noise Suppression（Krisp、Standard、None，unconfirmed）；Echo Cancellation；Automatic Gain Control（桌面標籤 unconfirmed）；Noise Reduction / Advanced Voice Activity（行動版，unconfirmed）；Quality of Service High Packet Priority；Attenuation（降低其他 App 音量，僅桌面，unconfirmed） | 未做 | 未排 | 系統層級回音消除（2026）為 Windows 專屬 |
| 語音設定：Windows 專屬 | Audio Subsystem（Standard/Legacy，僅 Windows）；Discord System Helper（實驗，讓快捷鍵在提權遊戲中運作，Windows 元件） | macOS 無此功能 | 未排 | |
| 語音設定：視訊與其他 | Video 區（Camera、Test Video、Always preview video、背景）；Hardware Acceleration；OpenH264 Video Codec 開關；Stream Previews；Soundboard 區；Voice Activity 通知（2026 恢復，標籤 unconfirmed）；Debugging（除錯記錄、Reset Voice and Video Settings）；設定頁改版 | 未做 | 未排 | |
| DAVE 端對端加密 | E2EE 涵蓋 DM、群組 DM、語音頻道與 Go Live，預設開且不可停用；Encrypted 指示（綠色鎖）；Voice Privacy Code（五位一組，有人進出就改變）；Stream Privacy Code；View Verification Code（每裝置）；Mark as Verified（名稱旁小圖示）；Persistent verification codes（Privacy & Safety > Encryption）；管理已驗證裝置 | 未做 | 未排 | 沒有 DAVE 就無法使用任何語音功能（見前述） |
| Clips | 僅 Windows（macOS、行動、主機皆無）；User Settings > Clips 開關；快捷鍵 Alt+C；長度 30 秒/60 秒/2 分；硬體編碼與儲存位置；Save Clip 按鈕與 overlay 彈窗；分享/編輯/刪除（修剪、遊戲/語音音訊、重新命名）；通知頻道成員（unconfirmed） | macOS 無此功能 | 未排 | 錄製為 Windows 專屬；播放他人 Clip 附件見 Message display |
| 語音訊息播放與 overlay | 桌面可播放語音訊息（Play），無法錄製或傳送；Overlay 語音 widget（Windows，Display Users 常駐/說話時、…）；Soundboard in overlay（Windows，Ctrl+`） | macOS 無此功能 | 未排 | 語音訊息播放本身在 Message display 另列（未做）；此列只記錄無法在 macOS 實作的部分（錄製與 overlay） |

## Threads and forums
| 功能群組 | 官方行為摘要 | velella 狀態 | 排程 | 備註 |
|---|---|---|---|---|
| 建立討論串 | 訊息「…」> Create Thread（可填標題與第一則，需 Create Public Threads）；從頂端 Threads 選單或輸入列「+」建立；從回覆建立（第一則不重複送出，2026 修正）；私人討論串開關（需 Create Private Threads）與「Anyone can invite」；公告頻道討論串（ANNOUNCEMENT_THREAD） | 未做 | 未排 | 訊息選單的 Create Thread 未列入 menu 計畫 |
| 編輯與生命週期 | 重新命名（建立者或 Manage Threads，無效或零寬名稱報錯）；Hide After Inactivity（1h/24h/3d 預設/1w）；Close / Open Thread（輸入即重開）；Lock thread（僅 Manage Threads 可重開）；Delete thread（確認，Enter 確認，2026）；討論串 slowmode（5 秒–6 小時，不繼承父頻道） | 未做 | 未排 | |
| 加入、通知與靜音 | Join / Leave Thread（輸入即加入、省略號選單）；通知設定（All/Mentions/Nothing）；Mute thread（被靜音的討論串在「new」跳轉被略過） | 未做 | 未排 | |
| 討論串於側欄與瀏覽 | 已加入的進行中討論串巢狀在父頻道下，關閉的移除；Threads browser（進行中與較舊，附預覽文字）；Active Threads view（全伺服器進行中，尊重劇透頻道，2026） | 部分 | 未排 | 側欄只列已加入且未封存的討論串，沒有討論串視圖與瀏覽器（status） |
| 討論串成員與提及 | 討論串成員 popout（版主移除、建立者移除）；使用者提及會加入該使用者，角色提及加入可檢視的角色成員 | 未做 | 未排 | |
| 討論串版面與訊息 | THREAD_CREATED 系統訊息（父頻道）與 THREAD_STARTER_MESSAGE（討論串頂端）；Split view（側欄面板，下拉含 Pins 等）；頻道搜尋包含討論串與封存討論串；討論串內投票；鎖定討論串唯讀（Forward 與 Bookmark 仍可用） | 未做 | 未排 | 系統訊息類型見 Message display |
| 論壇：建立與貼文 | 建立論壇頻道（類別上「+」> Forum，Set recommended permissions，僅 Community）；Create Post（標題、訊息、標籤、媒體）；貼文縮圖「Add to Post」（2026）；封存貼文加媒體會先解封（2026） | 未做 | 未排 | 論壇只有頻道列（status） |
| 論壇：標籤 | 每頻道最多 20 個（名稱 0–20 字、可含 emoji），每貼文最多 5 個；Moderated tags（僅 Manage Threads）；Require tags（REQUIRE_TAG）；右鍵貼文 Edit Tags；標籤篩選（All 或多個） | 未做 | 未排 | |
| 論壇：瀏覽 | 排序（Recent Activity / Creation Date）；版面 List View / Gallery View；Post Guidelines（4096 字）；貼文卡（標題、預覽、圖片、作者角色，預覽內劇透隱藏）；「New」徽章；新貼文是否標頻道未讀（NEW_FORUM_THREAD…，unconfirmed）；貼文側邊檢視；貼文搜尋（標題、Clear）；Older Posts（1h/24h/3d/1w 後） | 未做 | 未排 | |
| 論壇：管理與互動 | 預設反應與「React to post」按鈕；單一置頂貼文；Close Post / lock；貼文 slowmode 與預設討論串 slowmode；Follow post；AutoMod 關鍵字阻擋發文；Post templates（官方無文件，unconfirmed） | 未做 | 未排 | |
| LFG 貼文（2026 實驗） | Create Post（描述、標籤、語音聊天、Link Account 連動即時對局）；Live session card（空位與申請加入，需連結帳號）；Join（遊戲內接受請求或直接加入） | 未做 | 未排 | 限 ARC Raiders 官方伺服器的實驗，詳見 Channel list |
| 討論串與貼文狀態 | Active（側欄與 Threads browser，可發言）；Archived（隱藏於側欄，輸入重開，需 Send Messages…）；Locked（僅 Manage Threads 可解封，其他人唯讀）；Pinned（forum/media，PINNED 旗標，每頻道一個，置頂） | 部分 | 未排 | 目前只知道進行中的討論串；其他狀態沒有 UI |
| 媒體頻道 | 建立（「+」> Media，beta）；類相簿貼文（縮圖限一個）；訂閱制門檻（層級角色，非訂閱者看到模糊圖）；HIDE_MEDIA_DOWNLOAD_OPTIONS 旗標 | 未做 | 未排 | 只有頻道列（status） |

## User Settings
| 功能群組 | 官方行為摘要 | velella 狀態 | 排程 | 備註 |
|---|---|---|---|---|
| 設定入口與導覽 | 頭像旁齒輪開啟（通話中位置不同）；設定搜尋列；More 群組（What's New 在清單最後）；Language & Time 可由 Quick Switcher 開啟（2026-02，unconfirmed）；2025–2026 重整（桌面設定改版 stage 1、Notifications/Voice & Video/Clips/Streamer Mode 頁改版、App 設定合併為 Appearance/Accessibility/Chat/Streamer Mode/Advanced、Account 巢狀頁、分類改名 Activity→Games & Apps、Content & Social→Messaging Permissions） | 未做 | 未排 | 齒輪按鈕沒有動作，設定視窗不存在（status） |
| 我的帳號 | 帳號總覽欄位（使用者名稱、顯示名稱、Email 顯示/編輯、電話）；Password and Authentication（變更密碼、安全金鑰、驗證器、備份碼）；Account Status（年齡組別）；Account Standing；Account Removal（Disable/Delete） | 未做 | 未排 | 對應 Account & login 的各列 |
| Profiles：身分與外觀 | User Profile / Per-server Profiles 分頁；頭像（PNG/GIF，裁切，最近 6 張）；橫幅（Nitro，<10 MB，680×240）；About Me（300 字）；代名詞；Profile Theme；Display Name Style（Nitro 12 字型）；Avatar Decoration；Profile Effect；Nameplate；Profile Frame；Server Tag；Profile Widgets；Legacy Username Badge 開關；預覽與未儲存警告；最近活動與 Clear Game History；Wishlist | 未做 | 未排 | 詳見 Member list & profiles 的編輯群組 |
| Per-server profiles | 暱稱（免費）；per-server 頭像/橫幅/About Me/主題；per-server 代名詞（可清除，2026 年 5 月修正）；裝飾與特效；銘牌（2025）；顯示名稱樣式與外框 | 未做 | 未排 | |
| Messaging Permissions：內容與訊息 | 敏感內容過濾（Mature Sexual Media Filter、Graphic Media Filter）；DM 垃圾過濾；Message Requests 開關；Direct Message 權限（伺服器成員）；年齡限制指令（DM）；iOS 年齡限制伺服器（僅 iOS）；語言（關鍵字）過濾 | 未做 | 未排 | iOS 專屬項目在桌面不存在，已併入此列 |
| Messaging Permissions：好友與安全 | Friend Requests（Everyone/Friends of Friends/Server Members）；好友請求附註可見度（2026-08，unconfirmed）；好友請求警示（不可關閉）；已封鎖/已忽略清單；青少年安全警示；Encryption / Verified Devices；安全設定預設組（Balanced/Strict/Relaxed/Custom，unconfirmed） | 未做 | 未排 | 封鎖清單見 DMs and friends |
| Data & Privacy | Use Data to Improve Discord；個人化體驗開關；贊助內容個人化與 Manage Sponsored Content；Profile Privacy（Friends & All Servers / Friends & Small Servers Only ≤200 人）；Request Data（最多 30 天）；螢幕閱讀器偵測等 proto 開關（unconfirmed） | 未做 | 未排 | |
| Family Center | Account > Family Center（2026）；7 天活動儀表板；家長管理設定（敏感過濾、好友請求、伺服器 DM、訊息請求…）；消費上限；螢幕使用時間限制（結束前 15 分鐘通知）；回報時可通知家長 | 未做 | 未排 | |
| Authorized Apps 與 Connections | Authorized Apps（OAuth 與使用者安裝 App，Deauthorize）；Connections（新增帳號，Display on profile/details）；連線類型（Amazon Music、Bungie.net、Bluesky、Crunchyroll、Domain、eBay、Epic Games…）；Linked roles；第三方 App connections | 未做 | 未排 | |
| Clips 設定 | Enable Clips（Windows 搶先體驗，GPU 需求）、Clipping Keybind（Alt+C）、Clip Length、Hardware Encoding / Storage Location、Allow voice recording in clips（proto，unconfirmed） | macOS 無此功能 | 未排 | |
| Nitro、Boost、訂閱與帳單 | Nitro 頁（方案、Share Nitro）；Server Boost 分頁（Transfer Boost，7 天冷卻）；Subscriptions；Gift Inventory（未領取禮物、兌換 19 字元碼）；Billing（付款方式、交易） | 未做 | 未排 | 付款流程見 Nitro, monetization & extras |
| Appearance：主題 | Light/Ash/Dark/Onyx；Sync with computer；跨裝置同步主題；Dark Sidebar；Nitro 色彩主題與預覽；自訂主題（Nitro，最多 5 色、方向、強度）；伺服器主題偏好（實驗，3 boosts） | 部分 | 未排 | 只跟隨系統淺色/深色，並有 Liquid Glass 表面；Glass opacity 設定待辦（memory） |
| Appearance：密度與縮放 | UI Density（Compact/Default/Spacious，2025）；Message Spacing（Cozy/Compact）；Chat Font Scaling；Zoom Level（Reset to Default）；Message Group Spacing；App Icon（Nitro）；時鐘格式（12/24 小時，unconfirmed）；Show Members in Channel List | 部分 | 未排 | 時間格式跟隨系統 locale，其餘未做 |
| Accessibility：視覺 | Saturation 滑桿；High Contrast Mode；Sync contrast settings（僅 Windows 高對比）；Chat background contrast（2026-07）；Role Colors 三選項；Always Underline Links；Sync profile themes；Display Name Styles 開關 | 未做 | 未排 | 「Sync contrast settings」為 Windows 專屬，併入此列 |
| Accessibility：動態與朗讀 | Reduced Motion（Sync with computer 或手動）；GIF autoplay；Play animated emoji；Sticker animation；Text-to-Speech rate 與 /tts 允許；Keyboard Mode（Tab 焦點環、F6、訊息導覽）；Screen Readers 區（2026-04） | 部分 | 未排 | 遵守系統 Reduce Motion（status：done）；VoiceOver 標籤在重點檢視完成；其餘設定未做 |
| Voice & Video 頁 | Voice & Video 設定頁（含 Soundboard 音量與進場音效）；詳細項目見 Voice & video 的設定列 | 未做 | 未排 | |
| Chat（Text & Images） | Embeds and link previews（連結貼文時/直接上傳時）；Show spoiler content（點擊/我管理的伺服器/總是）；Mention suggestions；Show Favorites Server；Default reaction emoji（雙擊反應，2026）；Super Reactions 開關（2026-05）；Spellcheck；其他 proto 開關（顯示圖片描述、顯示 emoji 反應、轉換 emoticon…，unconfirmed） | 未做 | 未排 | |
| Notifications 與 Keybinds 頁 | Notifications 頁（桌面開關、音效、TTS、email，2026-03 改版，完整選項見 Notifications）；Keybinds（Add a Keybind：Push to Talk、Push To Mute、Toggle Mute、Toggle Deafen…）、移除（unconfirmed）、內建快捷鍵清單（Ctrl/Cmd+/）、OS 專屬 keybinds 合併（2026-08） | 未做 | 未排 | 快捷鍵清單見 Keyboard shortcuts |
| Language & Time | UI 語言選擇；@time 指令（時區自適應時間戳，2026-02） | 未做 | 未排 | 時間戳 `<t:>` 已能顯示，見 Message display |
| 系統設定（Windows / macOS） | Windows Settings 頁（桌面 OS 專屬）；Open Discord（登入時啟動）；Start Minimized（unconfirmed）；遊戲庫捷徑（桌面/開始選單，proto，unconfirmed） | 未做 | 未排 | macOS 對應為「登入時開啟」；Start Menu 捷徑為 Windows 專屬，併入此列，狀態以登入時啟動為準 |
| Streamer Mode | 主開關與橫幅；自動啟用（OBS/XSplit 執行中）；Hide Personal Information（Email、連線、discriminator…）；Hide Instant Invite Links；Disable Sounds；Disable Notifications；快捷鍵 Toggle Streamer Mode | 未做 | 未排 | |
| Advanced / Developer | Developer 頁（Advanced 併入，2026-05）；Developer Mode（Copy User/Server/Channel/Message ID…）；Hardware Acceleration；Client Info | 未做 | 未排 | Copy Message ID 目前不受 Developer Mode 控制（C1 選單固定顯示，待 M0 比對） |
| Games & Apps 與活動隱私 | 分類改名（2026-08）；Share my activity 全域開關；Automatically share my activity with（Friends & All Servers / Small Servers / …）；My Servers 個別覆寫；Join 權限（Allow friends to join my game、voice channel participants…）；Registered Games（逐遊戲偵測開關）；Per-app activity sharing（proto，unconfirmed） | 未做 | 未排 | |
| Game Overlay | Enable Overlay / Legacy Overlay（僅 Windows 10/11）；Overlay 快捷鍵（Shift+`）與 Lock；Overlay widgets（Notifications、Voice、Video、Activity、Streams、Soundboard）；遊戲內文字通知；頭像大小/顯示使用者/名稱 | macOS 無此功能 | 未排 | 官方 overlay 僅 Windows |
| More：What's New、HypeSquad、Log Out | What's New（App 內更新日誌）；HypeSquad 測驗/分頁已移除（徽章為傳承）；紅色 Log Out（清單最後） | 部分 | 未排 | 登出僅有 Sign Out 選單項，無設定頁的 Log Out |

## Server settings & moderation
| 功能群組 | 官方行為摘要 | velella 狀態 | 排程 | 備註 |
|---|---|---|---|---|
| 設定視窗外殼 | Server Settings 視窗（伺服器名稱下拉或右鍵開啟）；未儲存變更橫幅（Save Changes） | 未做 | 未排 | 目前 Server Settings 為 stub |
| Overview：基本 | 名稱與圖示（Manage Server）；系統訊息頻道（顯示頻道加類別，2026）；系統訊息開關（隱藏加入訊息、貼圖回應按鈕，unconfirmed）；Inactive (AFK) 頻道與逾時；預設通知設定（All/Only @mentions）；Show Boost progress bar | 未做 | 未排 | |
| Overview：外觀與擴充 | 橫幅（Level 2 靜態，3 動態）與邀請背景（Level 1）；Conversation Summaries 開關；Server Profile 分頁（名稱、圖示、橫幅色、自訂橫幅，2025）；Server Tag 分頁（4 字元，2025）；Server Tag Badge Packs（Creepy Crawlies 2 boosts、Pet 3、Plant 3、Flex 5）；Server Theme（3 boosts，實驗） | 未做 | 未排 | |
| 小工具、範本與網頁 | Widget（啟用、邀請頻道、widget.json）；Server Template（Generate Template、Copy link、Sync Template）；Custom Invite Link（Level 3，最多 25 字元）；Server Web Page（beta，Community > Server Web Pages）；Delete Server（僅擁有者，輸入伺服器名稱確認） | 未做 | 未排 | |
| 身分組：清單與建立 | 身分組清單（由上而下排序，@everyone 在最底作為範本）；Create Role；階層規則（Manage Roles 只能編輯低於自己最高身分組的）；Role member counts；Server Booster 身分組（自動、不可刪除或手動指派）；Managed roles（Bot、整合、訂閱，不可手動指派，可用於 onboarding，2026） | 未做 | 未排 | 伺服器身分組的資料已由 gateway 維護（GUILD_ROLE_*），僅用於顯示顏色 |
| 身分組：Display 分頁 | 名稱（成員列表即時更新）；顏色（預設色票與自訂，最高顏色身分組上色）；樣式（Solid/Gradient/Holographic，3 boosts，2025）；圖示（Level 2，ROLE_ICONS）；Display role members separately（hoist）；Allow anyone to @mention this role | 未做 | 未排 | |
| 身分組：權限與成員 | Permissions 分頁（General Server、Membership、Text Channel…分組開關）；Links 分頁（Linked Roles 條件）；Manage Members（加入成員、Remove Member）；省略號選單（View Server As Role、Delete Role，標籤 unconfirmed）；View Server As Role（僅擁有者/管理員，頂端控制列）；Linked Role 標籤；點擊角色提及列出最多 100 位成員；編輯已封鎖你的成員的身分組（Manage Roles）；頻道權限覆寫（Edit Channel > Permissions） | 未做 | 未排 | |
| 表情符號 | Emoji 分頁（Upload Emoji：JPEG/PNG/GIF/WEBP，<256 KB）；Emoji 欄位（Base/L1/L2/L3 為 50/100/150/250）；Emoji Studio（裁切、翻轉，2025） | 未做 | 未排 | status：GUILD_EMOJIS_UPDATE 未處理，F1 會讀取 guild emojis 作為資料 |
| 貼圖與音效板管理 | 貼圖分頁（Upload Sticker：PNG/APNG、名稱、欄位 5/15/30/60，超額停用）；Soundboard 分頁（檢視、編輯、移除、Add Sound）；表情符號權限（Create Expressions 與 Manage Expressions） | 未做 | 未排 | |
| 整合：Webhooks 與追蹤 | Integrations 單一頁面；Webhooks（誰何時加到哪個頻道）；Channels Followed（每個追蹤一個 webhook，可改名稱、頭像、目的頻道）；Twitch / YouTube 訂閱者身分組同步整合 | 未做 | 未排 | |
| 整合：Bots 與 Apps | Bots and Apps 清單（加入者與時間，開啟看權限）；App command permissions（依身分組/頻道/成員覆寫，unconfirmed）；App Directory（Add App）；Age-restricted commands | 未做 | 未排 | |
| Safety Setup：驗證與過濾 | Verification Level（None、Low 驗證 email、Medium 帳號超過 5 分鐘…）；Manually verify member；Explicit image filter（不掃描/掃描無身分組/全掃描）；Require 2FA for moderation（僅擁有者，MFA level Elevated）；Rules Screening（Members must accept…）；Age-restricted server（NSFW level：Default/Explicit/Safe/Age Restricted）；使用者端敏感內容過濾 | 未做 | 未排 | |
| Safety Setup：突襲與警示 | Raid Protection（ML 偵測、警示頻道）；CAPTCHA for suspicious joins（1 小時）；DM spam / raid detection（incident data）；Activity Alerts（頂端橫幅、離線推播、AutoMod…）；Alert-mode 系統訊息；Security Actions 對話框（Pause Invites 與 Pause DMs）；Pause Invites（Invites 分頁，Enable Invites） | 未做 | 未排 | 系統訊息呈現見 Message display |
| AutoMod：規則 | AutoMod 分頁（Manage Server 或 Administrator）；Commonly Flagged Words（Insults and Slurs、Sexual Content、Severe Profanity）；Custom Keyword（最多 6 條、每條 1,000 個關鍵字，每個 60 字元）；Regex（最多 10 個、每個 75 字元）；Block Spam Content（ML 垃圾與邀請垃圾）；Block Mention Spam（1–50 提及）；Block Words in Member Profile Names（Safety Setup > Members） | 未做 | 未排 | |
| AutoMod：動作與範圍 | Block message（自訂回覆 150 字，僅被阻擋者可見）；Send alert；Timeout（最長 4 週）；Block member interaction（隔離）；Exemptions（每規則最多 20 身分組與 50 頻道）；Scope（文字頻道、討論串、論壇貼文、語音文字聊天）；右鍵「Flag Word」捷徑 | 未做 | 未排 | |
| AutoMod：警示 | AUTO_MODERATION_ACTION 訊息（AutoMod 作者與 OFFICIAL 標籤）；警示快捷動作（timeout/kick/ban，unconfirmed）；Report Issues / 「Did we get it wrong?」；使用者名稱隔離系統訊息 | 未做 | 未排 | 警示卡渲染見 Message display |
| 稽核紀錄 | Audit Log 分頁（View Audit Log；依使用者與動作類型篩選）；項目：伺服器、頻道（原因，2026）、成員（Kick/prune/ban/update）、身分組、邀請/webhook/整合、表情、訊息（delete/bulk delete/pin/unpin）、Stage/活動/討論串、Apps 指令權限、AutoMod、monetization、onboarding/Server Guide、語音頻道狀態 | 未做 | 未排 | |
| 封鎖名單 | Bans 分頁（封鎖原因、搜尋，unconfirmed）；批次封鎖（Members 頁，Manage Server 加 Ban Members）；被封鎖者不能重新加入或重新申請 | 未做 | 未排 | |
| Members 頁 | 存取（頻道列的 Members）；成員表欄位（Name、Member Since、Joined Discord、Join Method…）；搜尋；排序；篩選；安全訊號（Unusual DM Activity、Unusual Account Activity、Timed Out、Quarantined…）；批次加身分組；列動作（More Options 或右鍵）；Prune（7/30 天未活動）；Member Applications 佇列（pending/approved/rejected）；即時移除（2026）；大型伺服器更快（2025） | 未做 | 未排 | 與 Member list & profiles 的 Members 頁重疊 |
| Community 設定 | Enable Community 流程（Administrator，需驗證 email 等）；Community Overview（規則頻道、Community Updates 頻道…）；Onboarding 設定；Server Guide 設定；Welcome Screen 設定（最多 5 頻道）；Server Insights（500 人以上，alpha）；Discovery 設定（Administrator，unconfirmed）；Student Hub 設定 | 未做 | 未排 | |
| 營利與 Boost | Server Subscriptions 分頁（開發者入口啟用 monetization 後）；Server Shop（Downloadables 與 Premium Roles，US 桌面/網頁實驗）；Creator monetization analytics；Boost Level 1/2/3（2/7/14 boosts 的 emoji、貼圖、音效、位元率、Go Live、Stage 視訊）；其他 Boost 福利（Server Tags 3、Enhanced Role Styles 3、Server Themes 3…）；等級寬限期 72 小時；Boost 系統訊息；Game Servers（2025 實驗） | 未做 | 未排 | 系統訊息見 Message display |
| 邀請與加入方式 | Invites 分頁（邀請者、代碼、使用次數、到期；X 撤銷）；Edit Channel > Invites（unconfirmed）；Access 分頁「How can people join your server?」（Invite Only/Apply to Join/Discovery）；申請問題；邀請到期選項（含 30 分鐘與 Never） | 未做 | 未排 | |
| 成員管理動作 | Kick（確認，unconfirmed）；Ban（刪除訊息歷史 0–7 天，unconfirmed）；Timeout（60 秒/5/10 分/1 小時/1 天/1 週）；被禁言成員的體驗（不能發言、新增反應、加入語音）；Remove Timeout；Change Nickname（Manage Nicknames）；Roles 子選單與迷你檔案「+」；Transfer Ownership；Mod View；Report / Flag to Mods（unconfirmed）；Report Raid（unconfirmed）；垃圾標記訊息跳轉；Copy User ID | 未做 | 未排 | 使用者右鍵選單未做（見 Member list & profiles） |
| 語音與 Stage 管理 | Server Mute / Server Deafen（Mute/Deafen Members，對封鎖你的成員也有效）；Move To / Disconnect（Move Members）；Stage moderation（Invite to Speak、Move to Audience、Disconnect from Stage…） | 未做 | 未排 | 需語音連線 |
| 權限旗標：訊息與反應 | ADD_REACTIONS、EMBED_LINKS、ATTACH_FILES、READ_MESSAGE_HISTORY、MENTION_EVERYONE、USE_EXTERNAL_EMOJIS、USE_EXTERNAL_STICKERS、MANAGE_MESSAGES、PIN_MESSAGES、SEND_MESSAGES（Send Messages and Create Posts）、SEND_TTS_MESSAGES、SEND_POLLS、SEND_VOICE_MESSAGES、BYPASS_SLOWMODE、USE_APPLICATION_COMMANDS、USE_EXTERNAL_APPS | 未做 | F1 | F1 在核心計算其中多個位元（含上列的 ADD_REACTIONS、USE_EXTERNAL_EMOJIS、MANAGE_MESSAGES、PIN_MESSAGES、SEND_MESSAGES、READ_MESSAGE_HISTORY、USE_APPLICATION_COMMANDS、USE_EXTERNAL_APPS）；EMBED_LINKS、ATTACH_FILES 等未列入 F1，仍未做 |
| 權限旗標：頻道與討論串 | VIEW_CHANNEL（頻道可見）、MANAGE_CHANNELS、CREATE_INSTANT_INVITE、MANAGE_THREADS、CREATE_PUBLIC_THREADS、CREATE_PRIVATE_THREADS、SEND_MESSAGES_IN_THREADS、MANAGE_WEBHOOKS、MANAGE_EVENTS、CREATE_EVENTS | 部分 | F1 | VIEW_CHANNEL 以 can_view_channel 套用在側欄；其餘 F1 計算 MANAGE_THREADS、SEND_MESSAGES_IN_THREADS，但尚無對應的 UI |
| 權限旗標：伺服器與管理 | ADMINISTRATOR、MANAGE_GUILD、KICK_MEMBERS、BAN_MEMBERS、VIEW_AUDIT_LOG、VIEW_GUILD_INSIGHTS、MANAGE_ROLES、MANAGE_GUILD_EXPRESSIONS、CREATE_GUILD_EXPRESSIONS、CHANGE_NICKNAME、MANAGE_NICKNAMES、MODERATE_MEMBERS、VIEW_CREATOR_MONETIZATION_ANALYTICS | 未做 | F1 | F1 計算 ADMINISTRATOR（全部權限），其餘無 UI |
| 權限旗標：語音與 Stage | PRIORITY_SPEAKER、STREAM（Video）、CONNECT、SPEAK、MUTE_MEMBERS、DEAFEN_MEMBERS、MOVE_MEMBERS、USE_VAD、REQUEST_TO_SPEAK、USE_EMBEDDED_ACTIVITIES、USE_SOUNDBOARD、USE_EXTERNAL_SOUNDS、SET_VOICE_CHANNEL_STATUS | 未做 | 未排 | |

## Keyboard shortcuts
| 功能群組 | 官方行為摘要 | velella 狀態 | 排程 | 備註 |
|---|---|---|---|---|
| 導覽：伺服器與頻道 | 切伺服器 Cmd+Opt+Up/Down；切頻道 Opt+Up/Down；切未讀頻道 Opt+Shift+Up/Down；切含提及的未讀頻道 Cmd+Shift+Opt+Up/Down；回到上一個文字頻道 Ctrl+B | 未做 | 未排 | macOS 綁定來自官方快捷鍵圖片的轉錄 |
| 導覽：語音與歷史 | 回到使用中的語音頻道 Cmd+Opt+A；跳回通話 Cmd+Shift+Opt+V；上一頁 Alt+Left；Cmd+[ / Cmd+]（unconfirmed）；滑鼠側鍵 4/5 前進後退 | 未做 | 未排 | |
| 導覽：全域動作 | Quick Switcher Cmd+K；關閉 Quick Switcher（Cmd+T 或 Esc，Cmd+T 為推論）；建立或加入伺服器 Cmd+Shift+N；建立群組 DM Cmd+Shift+T；開啟 User Settings Cmd+,；快捷鍵清單 Cmd+/；說明 Cmd+Shift+H | 未做 | 未排 | 目前只有 Sign Out 選單項 |
| 導覽：鍵盤模式與重排 | F6 / Shift+F6 切區塊；Tab / Shift+Tab 進入鍵盤模式；Return / Space 啟動焦點元素；方向鍵移動；Cmd+D 舉起重排（伺服器、頻道，方向鍵、Space/Return 放下、Esc 取消） | 部分 | 未排 | 右鍵選單內方向鍵導覽完成；其餘未做 |
| 閱讀與標記 | Esc 標記頻道已讀；Shift+Esc 標記伺服器已讀；PgUp/PgDown 捲動；Shift+PgUp 跳到最舊未讀；Cmd+Up/Down 瀏覽頻道歷史；Cmd+I Inbox；Cmd+Opt+@ 提及 popout；Cmd+Shift+E 標記頂端 Inbox 項已讀；Cmd+P 釘選 popout；Cmd+U 成員列表；Cmd+F / Cmd+Shift+F 搜尋；搜尋框內 Cmd+Delete（unconfirmed） | 未做 | 未排 | E、C、I 完成後對應的綁定仍需另排 |
| 輸入框快捷鍵 | Tab 聚焦輸入框；Up（或 Opt+Up）編輯上一則；Esc 取消訊息/回覆/編輯；Shift+Return 換行（unconfirmed）；Tab 或 Return 自動完成（unconfirmed）；Cmd+B/I/U；行內程式碼（未列出）；Cmd+E emoji、Cmd+G GIF、Cmd+S 貼圖選擇器；Cmd+Shift+U 上傳；Shift+點 GIF 只插入網址；Shift+F10 開啟輸入框選單；Home/End；Cmd+Return 發論壇貼文（unconfirmed）；刪除確認視窗 Return | 部分 | B（Up 編輯） | Return 送出、Esc 取消回覆完成；Up 編輯在 B；其餘未做 |
| 焦點訊息快捷鍵 | E 或 Up 編輯、R 回覆、Q 引用、+ 加反應、P 釘選、F 轉傳、Backspace 刪除、Cmd+C 複製內容、Opt+Return 標為未讀、Esc 回到輸入框；Shift+點刪除免確認（unconfirmed）；Opt+點訊息標為未讀（unconfirmed） | 未做 | 未排 | M0 會記錄官方訊息快捷鍵；綁定尚無切片 |
| 通話與語音快捷鍵 | 接聽 Cmd+Return；拒接 Esc；DM/群組開始通話 Ctrl+`（圖片轉錄）；切換靜音 Cmd+Shift+M；切換拒聽 Cmd+Shift+D；音效板 Cmd+Shift+B | 未做 | 未排 | 需語音連線 |
| App 與雜項快捷鍵 | 放大/縮小 Cmd+= / Cmd+-；重設縮放 Cmd+0（unconfirmed）；重新載入 Cmd+R（unconfirmed）；Vibing Wumpus 2.0 Cmd+Opt+Shift+W（彩蛋）；Quick Switcher 內復原 Cmd+Z | 未做 | 未排 | |
| 可設定的全域快捷鍵動作 | User Settings > Keybinds（僅桌面，Discord 失焦時全域有效）：Push to Talk（Normal/Priority）、Push to Mute、Toggle Mute/Deafen、Toggle VAD、Toggle Streamer Mode、Navigate Back/Forward、Switch to / Disconnect from Voice Channel、Toggle Go Live/Soundboard/Camera（unconfirmed）；多鍵序列、編輯時暫停、F10–F24 | 未做 | 未排 | 全域快捷鍵需輔助使用權限 |
| 僅 Windows 的快捷鍵 | Soundboard in game overlay、Toggle Overlay、Toggle Overlay Lock、Save Clip（官方標示 macOS n/a） | macOS 無此功能 | 未排 | Overlay 與 Clips 為 Windows 專屬 |

## App-level features
| 功能群組 | 官方行為摘要 | velella 狀態 | 排程 | 備註 |
|---|---|---|---|---|
| Quick Switcher | Cmd+K 跳到任一伺服器/頻道/DM；前綴 * 伺服器、@ DM/使用者、# 文字頻道、! 語音頻道；排序升級（2025）；可開啟設定頁（2026）；貼上邀請連結加入並開啟伺服器；搜尋列聚焦時 Cmd+K 也可用 | 未做 | 未排 | status：quick switcher 未做；前綴符號與 Search 區的記載有出入（官方文章 vs 第三方），兩處皆 unconfirmed 部分 |
| 連結與深層連結 | 訊息連結（Copy Message Link，格式 discord.com/channels/...，unconfirmed）；瀏覽器內 Discord 連結在桌面 App 開啟並保留返回導覽（2026）；discord:// URL scheme（unconfirmed）；訊息內導覽提及（`<id:customize>`、`<id:browse>`、`<id:guide>`、`<id:linked-roles>`、頻道/使用者提及） | 部分 | 未排 | Copy Message Link 完成；開啟連結/URL scheme/導覽提及動作未做 |
| 邀請 | Create Invite（頻道 hover，送給最近 DM 或複製連結，Edit Invite Link：到期等）；邀請連結格式（discord.gg/<code>、discord.com/invite/<code>、vanity，unconfirmed）；失效邀請狀態（過期、撤銷、無效）；群組 DM 邀請（滿員隱藏邀請按鈕，2026） | 未做 | 未排 | |
| 遊戲 Overlay | Game Overlay（僅 Windows 10/11，macOS 與 Linux 無）；新 overlay（widgets，不注入遊戲，可移動縮放）；overlay 文字聊天與通知（靜音頻道與 Streamer Mode 時隱藏）；釘選 widgets（Notifications/Voice/Video/Activity/Streams）；overlay 通話處理（快捷鍵接聽、hover Join Call / Decline） | macOS 無此功能 | 未排 | |
| Streamer Mode | 開關；偵測到 OBS 或 XSplit 自動啟停；快捷鍵；頂端橫幅（Disable 與 x）；Hide Personal Information（Email、連線、標籤）；Hide Instant Invite Links；Disable Sounds；Disable Notifications；設定位置併入合併頁面 | 未做 | 未排 | 與 User Settings 的 Streamer Mode 重複記載 |
| 主題與外觀 | 桌面基礎主題（Light 可選 Dark Sidebar、Ash、Dark、Onyx、Sync with computer）；行動版專屬主題（Midnight、行動版 Dark 回歸）；跨裝置同步；Nitro 色彩主題與自訂主題；Server Themes（實驗） | 部分 | 未排 | 跟隨系統淺色/深色；行動版主題在桌面無對應 |
| 外觀：密度與縮放 | UI Density（Compact/Default/Spacious）；Message display（Cozy/Compact）；Zoom level 滑桿與 Cmd+ / Cmd−；Chat Font Scaling；Message Group Spacing | 未做 | 未排 | |
| 無障礙：視覺與動態 | Saturation；High Contrast Mode（Sync Contrast Settings 僅 Windows，聊天背景遵守對比，2026）；Role Colors（名稱內/名稱旁/關閉）；Reduced Motion；GIF/emoji/貼圖動畫選項；Sync Profile Themes；Underline all links | 部分 | 未排 | 遵守系統 Reduce Motion；VoiceOver 標籤在重點檢視；其餘未做 |
| 朗讀與拼字 | Text-to-Speech（/tts，Notifications > Text-to-Speech：所有頻道/目前頻道/Never）；拼字檢查（OS 拼字器，右鍵開關、Add to Dictionary）；自訂 App 圖示（Nitro，桌面僅改 App 內圖示） | 未做 | J（Speak Message） | 「Speak Message」是訊息選單項（J），TTS 通知與拼字檢查未排 |
| 2025 桌面改版與視窗殼層 | 新色彩與插圖、可調整頻道列、Inbox 移到標題列；返回/前進按鈕（左上，滑鼠側鍵，2026）；What's New / Changelog（Settings > More）；更新指示（標題列 Downloading Update 圖示，Linux 自更新 2026）；Pop-out 視窗（通話、磚塊、串流、Stage、Activities，Stay On Top） | 未做 | 未排 | |
| 設定改版 | 2025 年底起：設定搜尋保留；分類改名（Activity→Games & Apps、Content & Social→Messaging Permissions）；各頁合併 | 未做 | 未排 | 重複記載於 User Settings |
| 登入啟動與工作列 | 登入時自動啟動（Windows 靜默開啟，2026，macOS 選項 unconfirmed）；Minimize to Tray / Start Minimized（Windows Settings，X 關閉至系統匣） | 未做 | 未排 | 系統匣為 Windows 概念，macOS 對應為選單列/Dock 行為，官方 macOS 行為 unconfirmed |
| 說明與下載 | Cmd+Shift+H Get Help（dis.gd/contact 表單）；Download apps（桌面下載頁；網頁客戶端 discord.com/login） | 未做 | 未排 | |
| 技術架構（參考） | Electron 多進程與 GPU process（2026 Electron 42） | macOS 無此功能 | 未排 | velella 為原生 SwiftUI 加 Rust 核心，此項僅作參考，不需對齊 |
| 通知與狀態橫幅 | Streamer Mode 橫幅；語音連線狀態提示（RTC Connecting / No Route / Waiting for Voice Server）；Krisp 自動關閉提示（高 CPU）；速率限制/驗證/連線中斷橫幅（官方來源找不到，unconfirmed） | 未做 | 未排 | status：重新連線/resume UI 未做 |
| 上傳與拖放 | 上傳失敗處理（具體錯誤訊息、重試暫時性錯誤、更新過期上傳 URL、空檔案）；拖放檔案到視窗上傳並預覽（comment、alt text 經 Modify） | 未做 | 未排 | |
| 無障礙拖放與選單 | 無障礙拖放重排（Cmd+D 舉起、方向鍵、Space/Enter 放下、Esc 取消）；右鍵選單涵蓋訊息、使用者、頻道、伺服器（Copy Server Info 子選單、Shift+F10） | 部分 | J | 訊息右鍵選單完成並由 J 補齊；使用者/頻道/伺服器選單與無障礙拖放未做 |
| 影片播放器統計 | 右鍵影片播放器 > Stats for Nerds（編碼、位元率、容器，2026） | 未做 | 未排 | 影片播放本身未做 |
| Active Now 與遊戲庫 | Friends 頁右欄 Active Now（unconfirmed 細節）；遊戲庫（桌面 App 已移除，unconfirmed）；遊戲偵測 / Registered Games（Add it!）；手動加入的遊戲顯示為活動 | 未做 | 未排 | |
| 開發者模式 | Developer Mode 開關（Advanced，現為 Developer 頁）：Copy User ID/Server ID/Channel ID/Message ID；其他功能 Copy Role ID、Application Test Mode（unconfirmed） | 部分 | 未排 | Copy Message ID 目前固定顯示，沒有 Developer Mode 開關 |
| 已讀與未讀快捷與 Inbox | Esc 標記頻道、Shift+Esc 標記伺服器（伺服器右鍵 Mark As Read 同時清除論壇貼文）；Alt+Enter 標為未讀（Opt+點訊息 unconfirmed）；Cmd+I Inbox（Mentions 與 Unreads 分頁…） | 未做 | E, I | 與 Message display / Search, Inbox & pins 重複記載 |
| 彩蛋 | Vibing Wumpus 2.0（Cmd+Opt+Shift+W，2026） | 未做 | 未排 | |

## Nitro, monetization & extras
| 功能群組 | 官方行為摘要 | velella 狀態 | 排程 | 備註 |
|---|---|---|---|---|
| Nitro 方案與限制 | 方案（Nitro、Nitro Basic 月繳/年繳；Nitro Classic 僅點數）；上傳上限（免費 20 MB（2026-08 起）、Basic 50 MB、Nitro 1 GB）；200 個伺服器；4,000 字訊息；Share Nitro（最多 3 個 2 週試用）；Nitro Squad（實驗，最多 6 帳號，AU/FR 桌面）；Nitro Credits（Orbs 專屬） | 未做 | 未排 | 字數與上傳上限的 Nitro 判斷需 F1 的 premium_type（只讀取，不含 UI） |
| Nitro 跨伺服器與反應福利 | 到處可用自訂/動態 emoji 與貼圖（Nitro 與 Basic）；Super Reactions 無限量（Nitro/Basic/Classic）；到處可用音效板與進場音效 | 未做 | P1, A | P1 的選擇器含 Nitro 鎖定；A 含 super reactions |
| Nitro 個人化福利 | 動態頭像、橫幅、個人檔案主題與 per-server 個人檔案；Nitro 徽章（tenure）；顏色與自訂主題；App 圖示；顯示名稱樣式（2025）；通話磚塊顏色符合個人檔案色（2026） | 未做 | 未排 | |
| Nitro 影音與經濟福利 | HD 串流；自訂視訊背景；2 個 Server Boost 加 30% 折扣；Shop 會員價；延伸 Quest 獎勵（2025）；每月 250 Orbs（2025-12）；1.2 倍 Orbs 倍率（2026-05-08 起） | 未做 | 未排 | |
| Server Boost 等級 | Level 1（2 boosts：100 emoji、15 貼圖、24 音效、128 kbps、720p60、邀請背景、動態圖示）、Level 2（7：150 emoji、30 貼圖、36 音效、256 kbps、1080p60、50 MB、角色圖示、橫幅）、Level 3（14：250 emoji、60 貼圖、48 音效、384 kbps、100 MB、自訂邀請連結）；額外福利（Server Tags 3、Enhanced Role Styles 3、Server Themes 3、250 MB 上傳…）；Booster 徽章/身分組/轉移（7 天冷卻） | 未做 | 未排 | 與 Server settings 的營利與 Boost 重複記載 |
| Shop：入口與瀏覽 | 桌面 Shop 分頁（DM 列表旁/伺服器列右側）、編輯個人檔案裝飾入口；搜尋、排序與篩選（2025-10；Browse 分頁 Sort By / Show Filters）；在地化價格；退款規則（收藏品不退款） | 未做 | 未排 | |
| Shop：商品類型 | Avatar Decorations（動態環，購買或 Quest/Orbs 取得，永久擁有）；Profile Effects（DISXCORE 僅 Nitro，遵守 Reduced Motion）；Nameplates；Profile Frames（Nitro 搶先體驗，Orbs 購買陸續推出，2026）；Bundles（折扣組合，單買一件即失去折扣）；Profile badges（Nitro、Booster、Quests、Orbs、Legacy Username、Bug Hunter…） | 未做 | 未排 | |
| Shop：會員價與 Wishlist | Nitro 會員價/會員專屬（Nitro 開始 5 天內購買…）；Wishlist（加入未擁有、非 Orbs 專屬、非 bundle 的商品）；Game Shops（Marvel Rivals、Rust、Bloons TD 6 的遊戲內商品）；轉傳 Shop 商品連結到聊天 | 未做 | D（轉傳） | 轉傳 Shop 連結屬一般訊息，不需 D |
| Orbs | Orbs 虛擬貨幣（不可購買、贈送、轉移、兌現；不過期…）；賺取（Orb-reward Quests、Nitro 每月 250、Orb Challenges 桌面實驗）；餘額顯示（Quest Home 與 Shop 右上，行動版頭像選單）；Orbs Exclusives 分頁（Nitro credits、Orbs 徽章、專屬裝飾/特效，僅桌面/瀏覽器）；雙價格顯示（金額與 Orbs，買得起時 Orbs 在前） | 未做 | 未排 | |
| Quests | Quest Home（Discover > Quests，桌面鍵盤可達，2026-10）；Quest bar（DM 列表 hover/展開：Accept Quest / Start Video Quest / Claim Reward）；Play Quests（桌面玩 X 分鐘、串流給語音中的好友、連結帳號完成…）；Video Quests（桌面/瀏覽器看預告，失焦暫停）；Cloud Play Quests（實驗）；獎勵（裝飾 2 個月（Nitro 除外）、Orbs、合作夥伴遊戲內代碼）；青少年上限（13–17 歲每日 3 個）；促銷控制（Hide This、Data & Privacy 個人化） | 未做 | 未排 | 「玩 X 分鐘」類 Quest 需遊戲偵測，在 macOS 的可行性 unconfirmed |
| 禮物與兌換碼 | Gift Nitro（輸入框右側 Gift 圖示或 Nitro 頁；Nitro/Basic、月/年）；禮物客製（DM：卡片設計、備註、音效板效果、emoji confetti）；禮物連結與領取（群組/頻道禮物形成可領取連結，embed「Open gift」→「Accept」）；Gift Shop items（收件者不可已擁有，重複在 48 小時…）；從 Wishlist 送禮；批次送禮（桌面實驗）；Gift Inventory（撤銷/重新產生連結）；Redeem Codes（19 字元）；季節性送禮促銷 | 未做 | 未排 | 與 DMs and friends 的禮物群組重疊 |
| 使用者安裝與伺服器安裝 App | User-installable apps（Add App 到帳號，所有伺服器、DM、群組 DM 可用，僅安裝者可見）；Server-installed apps（Manage Server，以成員形式出現）；App 標籤（In-App Purchases、Ad-Supported、Partner、Promoted）；App DMs（DM 一個 App，每 App 行動推播，unconfirmed） | 未做 | 未排 | |
| App Launcher 與指令 | App Launcher（輸入列旁：Recents、Collections、搜尋；語音通話內啟動 Activities）；Slash 與 context-menu 指令（「/」選擇器、右鍵 > Apps）；Image-editing apps（圖片上的 App Launcher，陸續推出） | 未做 | G（右鍵 Apps） | G 只涵蓋訊息指令；斜線選擇器、App Launcher 未排 |
| App Directory 與訂閱 | App Directory（桌面/瀏覽器：Discover 側欄 Apps 頁與 Server Settings > App Directory）；Premium App subscriptions（使用者或伺服器訂閱；耐久與消耗型一次性購買）；Activities（聊天或語音內啟動，PiP/pop-out，2025） | 未做 | 未排 | |

## Appendix: 2024–2026 feature timeline (official changelogs and patch notes)
| 功能群組 | 官方行為摘要 | velella 狀態 | 排程 | 備註 |
|---|---|---|---|---|
| 時間軸：投票與 Activities | Polls 與 Create Polls 權限（2024-05）；Activities 在文字頻道（2024-05）與 DM/群組 DM 文字聊天（2024-07）；Recommended Activities（2024-06）；Pop-out Activities（2025-09） | 未做 | 未排 | 見 Message display、Composer、Voice & video |
| 時間軸：App 與 DAVE | User-installable apps（2024-07）；App Launcher（2024-09）；圖片 App 編輯（2024-09）；E2EE 影音 DAVE（2024-09） | 未做 | 未排 | DAVE 是語音的先決條件 |
| 時間軸：登入與帳號 | Passkey sign-in（2024-09） | 部分 | 未排 | 內嵌網頁登入，Passkey 在 WKWebView 的可用性未驗證 |
| 時間軸：個人檔案 | 個人檔案預覽在 DM（2024-06）；桌面個人檔案改版與狀態泡泡（2024-07）；最近遊戲（2024-11）；重新整理的個人檔案與堆疊活動卡（2025-09）；頭像歷史（2025-06）；Mutual servers/friends 在 popout（2024-05）；WYSIWYG bio 編輯器（2026-08）；私人檔案（2026-08）；bio 190→300 字（2026-09）；Game Profile 頁（2026-06）；Frames（2026-08） | 未做 | 未排 | 對應 Member list & profiles |
| 時間軸：成員列表活動與標籤 | Member list activity（2024-08）；Server Tags（2025-06）；Enhanced Role Styles（2025-06）；Nameplates（2025-06）與 per-server nameplates（2025-11）；角色提及成員列表（2026-02/03）；新 Nitro 顯示名稱字型與 Gummy/Prism 特效（2026-09） | 未做 | 未排 | |
| 時間軸：外觀與設定 | 新基礎主題（2025-03）；UI density（2025-03）；Inbox 移到標題列（2025-03）；可調整頻道列（2025-03）；Custom Themes（2025-09）；Changelog 移到 Settings > More（2025-11）；返回/前進按鈕（2026-03）；Notifications/Voice & Video/Clips/Streamer Mode 改版（2026-03）；設定改名（2026-08）；音訊裝置圖示（2026-09） | 部分 | 未排 | 只跟隨系統淺色/深色；其餘未做 |
| 時間軸：訊息與嵌入 | Email markdown（2025-05）；AV1 影片（2025-07）；Tumblr embeds（2025-08）；TikTok 嵌入播放（2024-11）；Color profile preservation（2024-06） | 未做 | 未排 | |
| 時間軸：轉傳 | Message forwarding（2024-11）；轉傳支援 @silent（2026-07） | 未做 | D | |
| 時間軸：反應 | 建議 emoji 反應（2024-11）；Emoji editor 與選擇器 Add Emoji（2025-11） | 未做 | A, P1 | |
| 時間軸：釘選與權限 | Pins 50→250 與 Pin Messages 權限（2025-08/09）；Bypass Slowmode 權限（2026-02）；Pinned DMs 與 pinned channels（2026-06）；Use Category/Server Default 通知選項（2026-07，行動版） | 未做 | C（Pins）、F1（權限計算） | 後三項未排；Pins 與 Pin Messages 權限在 C 與 F1 |
| 時間軸：搜尋與 Quick Switcher | authorType: 篩選、markdown 詞彙高亮、論壇部分字詞搜尋（2025-09）；Quick Switcher 升級（2025-04） | 未做 | 未排 | |
| 時間軸：好友與 DM | Ignore users（2025-03）；Nitro friend passes（2024-06）；好友請求備註（2026-08）；Wave prompt（2026-06）；批次離開不活躍群組 DM（2026-09）；好友上線通知控制（2026-09）；DM 隱私覆寫指示（2026-10） | 未做 | 未排 | |
| 時間軸：Shop 與 Nitro | Shop bundles（2024-06）；免費上傳限制 10→20 MB（2026-08）；Discord Checkpoint（2025-12，unconfirmed，第三方報導） | 未做 | 未排 | |
| 時間軸：語音視訊 | Overlay widgets（2025-03）；集中式通話控制（2025-03）；可調整 PiP 通話視窗（2024-08）；相機開啟音效（2025-09）；語音頻道計時（2025-11）；串流縮放平移（2026-02/03）；Invite to Voice（2026-03）；語音邀請預覽（2026-06）；Stage PiP 與 Text-in-Voice 併用（2026-10） | 未做 | 未排 | Overlay widgets 僅 Windows，見 User Settings 的 Game Overlay |
| 時間軸：僅 Windows | 系統級回音消除（2026-09，Windows） | macOS 無此功能 | 未排 | |
| 時間軸：輸入與提及 | @time 指令（2026-02）；@game 提及（2026-08）；GIF 選擇器 Shift 點擊插入網址與右鍵複製連結（2026-10） | 未做 | 未排 | |
| 時間軸：影片播放器與頻道 | Stats for Nerds（2026-02）；Spoiler channels（2026-08） | 未做 | 未排 | |

## 摘要

| 區域 | 群組數 | 完成 | 部分 | 唯讀 | 未做 | macOS 無此功能 |
|---|---|---|---|---|---|---|
| Account & login | 18 | 0 | 6 | 0 | 12 | 0 |
| Server rail (guild sidebar) | 25 | 2 | 4 | 0 | 19 | 0 |
| Channel list | 39 | 3 | 8 | 0 | 28 | 0 |
| DMs and friends | 26 | 1 | 2 | 2 | 21 | 0 |
| Message display | 51 | 9 | 15 | 0 | 27 | 0 |
| Message actions | 21 | 1 | 2 | 0 | 18 | 0 |
| Composer | 21 | 1 | 2 | 0 | 15 | 3 |
| Reactions | 7 | 0 | 0 | 1 | 6 | 0 |
| Search, Inbox & pins | 14 | 0 | 1 | 0 | 13 | 0 |
| Member list & profiles | 19 | 2 | 2 | 0 | 15 | 0 |
| Presence & status | 8 | 1 | 1 | 0 | 6 | 0 |
| Notifications | 11 | 0 | 2 | 0 | 9 | 0 |
| Voice & video | 27 | 0 | 2 | 0 | 22 | 3 |
| Threads and forums | 13 | 0 | 2 | 0 | 11 | 0 |
| User Settings | 25 | 0 | 4 | 0 | 19 | 2 |
| Server settings & moderation | 28 | 0 | 1 | 0 | 27 | 0 |
| Keyboard shortcuts | 11 | 0 | 2 | 0 | 8 | 1 |
| App-level features | 22 | 0 | 5 | 0 | 15 | 2 |
| Nitro, monetization & extras | 14 | 0 | 0 | 0 | 14 | 0 |
| Appendix: 2024–2026 feature timeline (official changelogs and patch notes) | 17 | 0 | 2 | 0 | 14 | 1 |
| 合計 | 417 | 20 | 63 | 3 | 319 | 12 |

## 波次對應

功能缺口依 [roadmap.md](roadmap.md) 的里程碑排序；里程碑的先後（含使用者 2026-10-13 決定的「擴充系統排在 M3 之後」）不變，這裡只把未做的功能群組分進各里程碑，並列出 M3 剩下的波次。每一波開工前另寫計畫，計畫要涵蓋該波的全部功能群組，延後任何一項都要使用者決定。

| 里程碑 | 波次 | 涵蓋的功能群組 |
|---|---|---|
| M3 送訊息與互動 | selection（進行中） | N2 跨訊息選取、W1 連結與選取接上選單 |
| M3 | menu（計畫中，[menu.md](plans/menu.md)） | 完整訊息右鍵選單與 hover 列、reaction 寫入與 emoji 選擇器、編輯刪除、釘選、轉發、已讀狀態與未讀分隔線、書籤提醒與 Inbox、Apps 與 interaction、檢舉、Twemoji |
| M3 | composer（提議） | 附件上傳（拖放、貼上、spoiler、說明文字）、emoji／GIF／貼圖選擇器、mention／頻道／emoji 自動完成、slash 指令、字數上限、送出 typing、輸入框狀態（slowmode、timeout、無權限、封存討論串）、投票建立 |
| M3 | message types（提議） | 全部系統訊息類型、轉發與 snapshot 顯示、貼圖、投票顯示與投票、語音訊息與影音播放、附件下載、bot 元件（按鈕、選單、Components v2）、ephemeral 訊息、Jump to Present、compact 模式 |
| M4 擴充系統 | （roadmap 原定） | plugin API |
| M5 伺服器深度 | navigation、people、notifications（提議） | 分類收合、頻道／伺服器／使用者右鍵選單、Browse Channels、討論串與論壇瀏覽、搜尋、⌘K 快速切換、快捷鍵全表；個人檔案 popout 與完整檔案、好友頁、訊息請求、建立 DM、自己的狀態與自訂狀態、activity 顯示；系統通知、Dock 徽章、各頻道通知設定 |
| M6 語音與視訊 | voice（roadmap 原定） | 語音頻道（DAVE 必要，否則 close code 4017）、視訊、螢幕分享、舞台、Soundboard |
| M7 打磨與發佈 | settings、account（提議） | 使用者設定各分頁、多帳號、原生登入、登出時撤銷 session、Nitro 外觀（名稱樣式、頭像裝飾、個人檔案效果） |
| 未排 | 伺服器設定與管理 | 伺服器設定、角色、AutoMod、稽核紀錄、踢出／封鎖／timeout 對話框 |
