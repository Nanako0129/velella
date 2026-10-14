import Foundation

/// Discord-localised times (docs/plans/markdown.md, K4). ICU through `DateFormatter` and
/// `RelativeDateTimeFormatter` with `Locale(identifier:)`: the macOS preferred language, none of the
/// macOS user overrides (12/24-hour, date order). Styles whose official output differs from ICU
/// get an explicit pattern once K0 records it.
enum DiscordTime {
    /// `unix` arrives within +-8.64e12 s from core (K1 D3); clamped here too, so no value that
    /// reaches a formatter is out of Foundation's `Date` range, and no `Int` conversion happens.
    static let unixLimit: Int64 = 8_640_000_000_000

    /// `R` is English whatever the locale: the official client shows "5 years ago" in
    /// zh-TW (K0, `refs/discord-ground-truth/captures/k0-times-markdown.png`). Singular and "now"
    /// wording stay ICU's until a capture shows Discord's.
    static let relativeLocale = Locale(identifier: "en-US")

    static func date(unix: Int64) -> Date {
        Date(timeIntervalSince1970: Double(min(max(unix, -unixLimit), unixLimit)))
    }

    static func format(unix: Int64, style: TimestampStyle, locale: Locale, now: Date, timeZone: TimeZone = .current) -> String {
        format(date: date(unix: unix), style: style, locale: locale, now: now, timeZone: timeZone)
    }

    static func format(date: Date, style: TimestampStyle, locale: Locale, now: Date, timeZone: TimeZone = .current) -> String {
        let (dateStyle, timeStyle): (DateFormatter.Style, DateFormatter.Style)
        switch style {
        case .shortTime: (dateStyle, timeStyle) = (.none, .short)
        case .mediumTime: (dateStyle, timeStyle) = (.none, .medium)
        case .shortDate: (dateStyle, timeStyle) = (.short, .none)
        case .longDate: (dateStyle, timeStyle) = (.long, .none)
        case .longDateShortTime: (dateStyle, timeStyle) = (.long, .short)
        case .fullDateShortTime: (dateStyle, timeStyle) = (.full, .short)
        case .shortDateShortTime: (dateStyle, timeStyle) = (.short, .short)
        case .shortDateMediumTime: (dateStyle, timeStyle) = (.short, .medium)
        case .relative:
            let formatter = RelativeDateTimeFormatter()
            formatter.locale = relativeLocale
            formatter.dateTimeStyle = .numeric
            formatter.unitsStyle = .full
            return formatter.localizedString(for: date, relativeTo: now)
        }
        return formatter(locale, timeZone, dateStyle, timeStyle).string(from: date)
    }

    /// The hover tip of every timestamp: the `F` style (v2: "Wednesday, April 21, 2021 at 5:20 AM").
    static func tooltip(unix: Int64, locale: Locale, now: Date, timeZone: TimeZone = .current) -> String {
        tooltip(date: date(unix: unix), locale: locale, now: now, timeZone: timeZone)
    }

    static func tooltip(date: Date, locale: Locale, now: Date, timeZone: TimeZone = .current) -> String {
        format(date: date, style: .fullDateShortTime, locale: locale, now: now, timeZone: timeZone)
    }

    // MARK: Relative refresh

    static let secondRefresh: TimeInterval = 1
    static let halfMinuteRefresh: TimeInterval = 30
    static let quarterHourRefresh: TimeInterval = 15 * 60

    /// How often an `R` timestamp must be rebuilt to stay right: every second while the text counts
    /// seconds, every 30 s while it counts minutes, every 15 min after that.
    static func refreshInterval(for unix: Int64, now: Date) -> TimeInterval {
        let distance = abs(date(unix: unix).timeIntervalSince(now))
        if distance < 60 { return secondRefresh }
        if distance < 3600 { return halfMinuteRefresh }
        return quarterHourRefresh
    }

    // MARK: Message header

    /// PROVISIONAL, pending the K0 capture of an older day's header in the official client: the
    /// form for a message that is neither from today nor from yesterday ("<short date> <short
    /// time>"). The one place to change once K0 records the real text or pattern.
    static let olderHeaderStyle: (date: DateFormatter.Style, time: DateFormatter.Style) = (.short, .short)

    /// "<short time>" today (the official client shows `下午2:23`, K0), "Yesterday at <short
    /// time>", else the older-day form, in `locale`. `systemNow` is the clock ICU's relative words
    /// are measured against; only tests pass another.
    static func header(date: Date, now: Date, locale: Locale, timeZone: TimeZone = .current, systemNow: Date = .now) -> String {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = timeZone
        let daysBack = [0, -1].first { offset in
            calendar.date(byAdding: .day, value: offset, to: now).map { calendar.isDate(date, inSameDayAs: $0) } ?? false
        }
        guard let daysBack else {
            return formatter(locale, timeZone, olderHeaderStyle.date, olderHeaderStyle.time).string(from: date)
        }
        if daysBack == 0 { return formatter(locale, timeZone, .none, .short).string(from: date) }
        // ICU's "Today"/"Yesterday" follow the system clock, not `now`: format a stand-in at the same
        // time of day on the matching day of the system clock.
        let time = calendar.dateComponents([.hour, .minute, .second], from: date)
        let day = calendar.date(byAdding: .day, value: daysBack, to: systemNow) ?? systemNow
        let standIn = calendar.date(bySettingHour: time.hour ?? 0, minute: time.minute ?? 0, second: time.second ?? 0, of: day) ?? day
        let relative = formatter(locale, timeZone, .medium, .short)
        relative.doesRelativeDateFormatting = true
        return relative.string(from: standIn)
    }

    private static func formatter(_ locale: Locale, _ timeZone: TimeZone, _ date: DateFormatter.Style, _ time: DateFormatter.Style) -> DateFormatter {
        let formatter = DateFormatter()
        formatter.locale = locale
        formatter.timeZone = timeZone
        formatter.dateStyle = date
        formatter.timeStyle = time
        return formatter
    }
}
