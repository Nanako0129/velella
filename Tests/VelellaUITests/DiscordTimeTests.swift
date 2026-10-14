import Foundation
import Testing
@testable import VelellaUI

// Expected strings are what Foundation's ICU produces for the fixed instant below (UTC), recorded
// on macOS 26. CLDR puts U+202F (narrow no-break space) before AM/PM in English. The zh-TW rows
// are ICU's, pending the K0 capture of the official client.

private let utc = TimeZone(identifier: "UTC")!
/// 2021-04-20 21:20:30 UTC.
private let instant: Int64 = 1_618_953_630
private let at = Date(timeIntervalSince1970: 1_618_953_630)
private let nn = "\u{202F}"
/// zh-TW puts U+2009 (thin space) between the short date and the time.
private let thin = "\u{2009}"

private func all(_ locale: String) -> [String] {
    let styles: [TimestampStyle] = [
        .shortTime, .mediumTime, .shortDate, .longDate, .longDateShortTime, .fullDateShortTime,
        .shortDateShortTime, .shortDateMediumTime,
    ]
    return styles.map { DiscordTime.format(unix: instant, style: $0, locale: Locale(identifier: locale), now: at, timeZone: utc) }
}

@Test func everyStyleEnUS() {
    #expect(all("en-US") == [
        "9:20\(nn)PM", "9:20:30\(nn)PM", "4/20/21", "April 20, 2021",
        "April 20, 2021 at 9:20\(nn)PM", "Tuesday, April 20, 2021 at 9:20\(nn)PM",
        "4/20/21, 9:20\(nn)PM", "4/20/21, 9:20:30\(nn)PM",
    ])
}

@Test func everyStyleEnGB() {
    #expect(all("en-GB") == [
        "21:20", "21:20:30", "20/04/2021", "20 April 2021",
        "20 April 2021 at 21:20", "Tuesday, 20 April 2021 at 21:20",
        "20/04/2021, 21:20", "20/04/2021, 21:20:30",
    ])
}

/// ICU, pending K0.
@Test func everyStyleZhTW() {
    #expect(all("zh-TW") == [
        "晚上9:20", "晚上9:20:30", "2021/4/20", "2021年4月20日",
        "2021年4月20日 晚上9:20", "2021年4月20日 星期二 晚上9:20",
        "2021/4/20\(thin)晚上9:20", "2021/4/20\(thin)晚上9:20:30",
    ])
}

@Test func timeZoneShiftsTheDay() {
    let taipei = TimeZone(identifier: "Asia/Taipei")!
    #expect(DiscordTime.format(unix: instant, style: .shortDateShortTime, locale: Locale(identifier: "en-US"), now: at, timeZone: taipei)
        == "4/21/21, 5:20\(nn)AM")
}

@Test func relativePastAndFuture() {
    let en = Locale(identifier: "en-US")
    let fourYears = Calendar(identifier: .gregorian).date(byAdding: .year, value: 4, to: at)!
    #expect(DiscordTime.format(unix: instant, style: .relative, locale: en, now: fourYears) == "4 years ago")
    #expect(DiscordTime.format(unix: instant + 7200, style: .relative, locale: en, now: at) == "in 2 hours")
    #expect(DiscordTime.format(unix: instant, style: .relative, locale: en, now: at.addingTimeInterval(7200)) == "2 hours ago")
    // English whatever the locale (K0: "5 years ago" in zh-TW).
    #expect(DiscordTime.format(unix: instant, style: .relative, locale: Locale(identifier: "zh-TW"), now: at.addingTimeInterval(7200)) == "2 hours ago")
}

@Test func tooltipIsTheFullDate() {
    let en = Locale(identifier: "en-US")
    #expect(DiscordTime.tooltip(unix: instant, locale: en, now: at, timeZone: utc) == "Tuesday, April 20, 2021 at 9:20\(nn)PM")
    #expect(DiscordTime.tooltip(unix: instant, locale: en, now: at, timeZone: utc)
        == DiscordTime.format(unix: instant, style: .fullDateShortTime, locale: en, now: at, timeZone: utc))
    #expect(DiscordTime.tooltip(date: at, locale: en, now: at, timeZone: utc) == DiscordTime.tooltip(unix: instant, locale: en, now: at, timeZone: utc))
}

@Test func refreshIntervalByDistance() {
    #expect(DiscordTime.refreshInterval(for: instant, now: at) == DiscordTime.secondRefresh)
    #expect(DiscordTime.refreshInterval(for: instant, now: at.addingTimeInterval(59)) == DiscordTime.secondRefresh)
    #expect(DiscordTime.refreshInterval(for: instant, now: at.addingTimeInterval(-59)) == DiscordTime.secondRefresh)
    #expect(DiscordTime.refreshInterval(for: instant, now: at.addingTimeInterval(60)) == DiscordTime.halfMinuteRefresh)
    #expect(DiscordTime.refreshInterval(for: instant, now: at.addingTimeInterval(3599)) == DiscordTime.halfMinuteRefresh)
    #expect(DiscordTime.refreshInterval(for: instant, now: at.addingTimeInterval(3600)) == DiscordTime.quarterHourRefresh)
    #expect(DiscordTime.refreshInterval(for: instant + 86_400, now: at) == DiscordTime.quarterHourRefresh)
}

@Test func extremeUnixDoesNotTrap() {
    let limit = DiscordTime.unixLimit
    #expect(limit == 8_640_000_000_000)
    for locale in ["en-US", "zh-TW"] {
        for unix in [limit, -limit, Int64.max, Int64.min] {
            for style in [TimestampStyle.shortTime, .mediumTime, .shortDate, .longDate, .longDateShortTime, .fullDateShortTime, .shortDateShortTime, .shortDateMediumTime, .relative] {
                #expect(!DiscordTime.format(unix: unix, style: style, locale: Locale(identifier: locale), now: at, timeZone: utc).isEmpty)
            }
            #expect(!DiscordTime.tooltip(unix: unix, locale: Locale(identifier: locale), now: at).isEmpty)
            #expect(DiscordTime.refreshInterval(for: unix, now: at) == DiscordTime.quarterHourRefresh)
        }
    }
    // Past the limit clamps to it.
    #expect(DiscordTime.date(unix: Int64.max) == DiscordTime.date(unix: limit))
}

@Test func timestampCaseUsesTheContext() {
    let context = RenderContext(now: at.addingTimeInterval(7200), locale: Locale(identifier: "en-GB"))
    let f = MarkdownText.timestamp(unix: instant, style: .shortTime, context: context)
    #expect(f.text == DiscordTime.format(unix: instant, style: .shortTime, locale: context.locale, now: context.now))
    #expect(f.hoverTip == DiscordTime.tooltip(unix: instant, locale: context.locale, now: context.now))
    #expect(f.refreshEvery == nil)
    let r = MarkdownText.timestamp(unix: instant, style: .relative, context: context)
    #expect(r.text == "2 hours ago")
    #expect(r.refreshEvery == DiscordTime.quarterHourRefresh)
}

// MARK: Message header

private func header(_ locale: String, now: Date, timeZone: TimeZone = utc) -> String {
    DiscordTime.header(date: at, now: now, locale: Locale(identifier: locale), timeZone: timeZone)
}

@Test func headerTodayYesterdayOlderEnUS() {
    // Today: the time only (K0: `下午2:23`).
    #expect(header("en-US", now: at.addingTimeInterval(7200)) == "9:20\(nn)PM")
    #expect(header("en-US", now: at.addingTimeInterval(86_400)) == "Yesterday at 9:20\(nn)PM")
    // PROVISIONAL (olderHeaderStyle), pending K0.
    #expect(header("en-US", now: at.addingTimeInterval(5 * 86_400)) == "4/20/21, 9:20\(nn)PM")
}

/// ICU, pending K0.
@Test func headerTodayYesterdayOlderZhTW() {
    #expect(header("zh-TW", now: at.addingTimeInterval(7200)) == "晚上9:20")
    #expect(header("zh-TW", now: at.addingTimeInterval(86_400)) == "昨天 晚上9:20")
    #expect(header("zh-TW", now: at.addingTimeInterval(5 * 86_400)) == "2021/4/20\(thin)晚上9:20")
}

@Test func headerDayFollowsTheTimeZone() {
    // 21:20 UTC is 05:20 the next day in Taipei: "now" 1 h later is still the same Taipei day.
    let taipei = TimeZone(identifier: "Asia/Taipei")!
    #expect(header("en-US", now: at.addingTimeInterval(3600), timeZone: taipei) == "5:20\(nn)AM")
    // ... but yesterday in UTC terms is two Taipei days back from 2 days later.
    #expect(header("en-US", now: at.addingTimeInterval(2 * 86_400), timeZone: taipei) == "4/21/21, 5:20\(nn)AM")
}

@Test func headerOfAFutureMessageIsTheOlderForm() {
    #expect(header("en-US", now: at.addingTimeInterval(-3 * 86_400)) == "4/20/21, 9:20\(nn)PM")
}

// MARK: K0 capture (zh-TW, Asia/Taipei; refs/discord-ground-truth/captures/k0-times-markdown.png)

/// The official client's strings for sample 1 at 05:20 Taipei. ICU's s/S separator is U+2009,
/// which the capture cannot tell from a space at its size.
@Test func everyStyleMatchesTheK0CaptureInTaipei() {
    let taipei = TimeZone(identifier: "Asia/Taipei")!
    let zh = Locale(identifier: "zh-TW")
    let styles: [TimestampStyle] = [
        .shortTime, .mediumTime, .shortDate, .longDate, .longDateShortTime, .fullDateShortTime,
        .shortDateShortTime, .shortDateMediumTime,
    ]
    #expect(styles.map { DiscordTime.format(unix: instant, style: $0, locale: zh, now: at, timeZone: taipei) } == [
        "清晨5:20", "清晨5:20:30", "2021/4/21", "2021年4月21日",
        "2021年4月21日 清晨5:20", "2021年4月21日 星期三 清晨5:20",
        "2021/4/21\(thin)清晨5:20", "2021/4/21\(thin)清晨5:20:30",
    ])
    // "f（下午）": 1618996830 is 17:20 in Taipei.
    #expect(DiscordTime.format(unix: 1_618_996_830, style: .longDateShortTime, locale: zh, now: at, timeZone: taipei) == "2021年4月21日 下午5:20")
    let fiveYears = Calendar(identifier: .gregorian).date(byAdding: .year, value: 5, to: at)!
    #expect(DiscordTime.format(unix: instant, style: .relative, locale: zh, now: fiveYears, timeZone: taipei) == "5 years ago")
}

/// K4 verification A2: an `R` pill is built from `context.now`, not the wall clock. The instant is
/// years before the real now, so a wall-clock build would read "years ago" and refresh every 15 min.
@Test func relativeRefreshFollowsTheContextNow() {
    let context = RenderContext(now: at.addingTimeInterval(30), locale: Locale(identifier: "zh-TW"))
    let r = MarkdownText.timestamp(unix: instant, style: .relative, context: context)
    #expect(r.text == "30 seconds ago")
    #expect(r.refreshEvery == DiscordTime.secondRefresh)
    let later = MarkdownText.timestamp(unix: instant, style: .relative, context: RenderContext(now: at.addingTimeInterval(59), locale: context.locale))
    #expect(later.text == "59 seconds ago" && later.refreshEvery == DiscordTime.secondRefresh)
}
