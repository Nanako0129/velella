# velella UI 設計原則與偏離 Swiftcord v2 之處

## 文件目的

velella 的介面以 Swiftcord v2 的實際畫面為主要參考（截圖在 `refs/swiftcord-v2-screens/`，只在本機、不進 git），能沿用 Swiftcord 舊版程式碼（GPL-3.0）的部分優先沿用，其餘照標準 Discord 版面，一律用原生 SwiftUI 元素與 Liquid Glass。這份文件記錄使用者決定要「比 Swiftcord v2 做得更好」的地方，讓後續每個 UI slice 都照著做。

## 共通規則

| 規則 | 說明 |
|---|---|
| 參考來源 | 先看實際渲染畫面，不從缺少該功能的程式碼推測 |
| 程式碼來源 | Swiftcord 舊版的 view 優先沿用（檔頭註明出處、列入 THIRD_PARTY_NOTICES.md）；DiscordKit 沒有授權，一行都不碰 |
| 元素 | 原生 SwiftUI（NavigationSplitView、List、Form、系統材質、Liquid Glass），不用 Introspect 或旋轉 hack |
| 視覺參數 | 全部放在具名常數，附一行「為什麼」 |
| 收斂方式 | 使用者一輪一輪審查，每輪只改被指出的地方 |

## 偏離 Swiftcord v2 的決定

| 項目 | Swiftcord v2 的現況 | velella 的做法 | 決定日期 |
|---|---|---|---|
| 個人檔案 popover 的文字對比 | 背景依個人檔案主題色漸層著色後，部分文字（狀態、帳號選單、底部按鈕）顏色太淺，看不清楚（`e-profile-popover-self.png`） | 文字顏色依實際背景亮度決定，主要文字與次要文字都要達到 WCAG AA 對比（4.5:1）；主題色只影響背景，不能讓文字變淡到讀不出來 | 2026-10-13 |
| Discord 特殊樣式字體（Display Name Styles） | 顯示名稱一律用系統字體，Nitro 使用者設定的字體、顏色、動態效果都沒有呈現 | 呈現 Display Name Styles 的字體、顏色與效果；字元不支援時退回預設字體。欄位名稱尚未確認（官方文件沒有公開），要在 S3 實際連線後從 user payload 確認，字體檔的取得方式與授權也一併確認 | 2026-10-13 |
| 輸入框 `+` 選單 | 原生窄版選單（Use Apps、Create Poll、Upload to Host、Files），見 `g-composer-plus-menu.png` | 還原 Discord 的寬版選單：每列有圖示、列高較大、間距寬鬆，但材質用 Liquid Glass，不用 Discord 的深灰底 | 2026-10-13 |
| 回覆預覽裡的自訂 emoji | 顯示原始標記 `<:name:id>`（`g-composer-reply.png`） | 渲染成 emoji 圖片 | 2026-10-13 |
| 視窗透明度 | 側欄、成員列表、toolbar 帶都接近不透明的中性深灰，桌布幾乎不透出來 | 保留透明感，而且每一區都一樣透：視窗底材維持透明，訊息欄用半透明深色底，並延伸到 toolbar 底下，讓桌布色調均勻地透進整個視窗，只是訊息欄比較深 | 2026-10-13 |

## 相關的 Discord 外觀功能

nameplate（成員列背景圖）與頭像裝飾也是 Discord 的外觀功能，Serein 有實作（`refs/serein/crates/ui/src/avatars/cosmetics.rs`，MIT/Apache），可在做成員列表與個人檔案時一併參考。
