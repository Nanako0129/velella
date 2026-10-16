import AppKit
import Foundation
import Testing
@testable import VelellaUI

// Slice P1a: the bundled emoji catalog (emoji-toolkit 11.0.0 filtered to Emoji 16, Discord's names) and
// the Twemoji 16.0.1 artwork. Expected names and orders were checked against Discord's own data when the
// catalog was generated; the lists below are written out so a regression in the generator shows here.

private let catalog = EmojiCatalog.shared

private func e(_ name: String) -> EmojiEntry {
    guard let x = catalog.entry(named: name) else { fatalError("no emoji named \(name)") }
    return x
}

@Test func twemojiFileNameRule() {
    #expect(TwemojiFile.name(forSequence: "\u{2764}\u{FE0F}") == "2764")  // FE0F dropped
    #expect(TwemojiFile.name(forSequence: "\u{2764}") == "2764")
    #expect(TwemojiFile.name(forSequence: "4\u{FE0F}\u{20E3}") == "34-20e3")  // keycap, no padding
    #expect(TwemojiFile.name(forSequence: "\u{2764}\u{FE0F}\u{200D}\u{1F525}") == "2764-fe0f-200d-1f525")  // ZWJ keeps FE0F
    #expect(TwemojiFile.name(forSequence: "\u{1F1EF}\u{1F1F5}") == "1f1ef-1f1f5")  // flag
    // skin-toned ZWJ family: man, tone 3, ZWJ, woman, tone 2, ZWJ, girl, ZWJ, boy
    #expect(TwemojiFile.name(forSequence: "\u{1F468}\u{1F3FD}\u{200D}\u{1F469}\u{1F3FC}\u{200D}\u{1F467}\u{200D}\u{1F466}")
        == "1f468-1f3fd-200d-1f469-1f3fc-200d-1f467-200d-1f466")
    #expect(TwemojiFile.name(forSequence: "\u{00A9}\u{FE0F}") == "a9")  // no padding
    #expect(TwemojiFile.name(forSequence: "\u{1F441}\u{FE0F}\u{200D}\u{1F5E8}\u{FE0F}") == "1f441-200d-1f5e8")  // the exception
}

@Test func catalogSizesMatchDiscord() {
    let sizes = EmojiCategory.allCases.map { catalog.entries(in: $0).count }
    #expect(sizes == [511, 214, 130, 122, 131, 226, 328, 270])
    #expect(catalog.entries.count == 3812)
    // every entry resolves to itself by sequence and by primary name
    for x in catalog.entries {
        #expect(catalog.entry(forSequence: x.sequence) == x)
        #expect(catalog.entry(named: x.name) == x)
    }
}

@Test func everyEntryHasAnImageFile() throws {
    let folder = try #require(EmojiImageProvider.bundledFolder)
    var missing: [String] = []
    for x in catalog.entries
    where !FileManager.default.fileExists(atPath: folder.appendingPathComponent(x.imageName + ".svg").path) {
        missing.append(x.imageName)
    }
    #expect(missing.isEmpty, "\(missing.prefix(10))")
}

@Test func overridesAreApplied() {
    // added aliases
    #expect(e("number_0").sequence == "0\u{FE0F}\u{20E3}")
    #expect(e("number_10") == e("keycap_ten"))
    #expect(e("bride_with_veil") == e("woman_with_veil"))
    #expect(e("iphone") == e("mobile_phone"))
    #expect(e("pinata") == e("piñata"))
    #expect(e("actual_football") == e("soccer"))
    #expect(e("c") == e("copyright") && e("r") == e("registered"))
    // hyphens become underscores
    #expect(catalog.entry(named: "blond-haired_woman") == nil)
    #expect(e("blond_haired_woman").sequence.unicodeScalars.first?.value == 0x1F471)
    #expect(catalog.entry(named: "e_mail") != nil && catalog.entry(named: "e-mail") == nil)
    #expect(catalog.entry(named: "non_potable_water") != nil)
    // Discord's own spellings
    #expect(e("+1").name == "thumbsup" && e("-1").name == "thumbsdown")
    #expect(e("_1_tone1").name == "thumbsdown_tone1")
    #expect(e("flag_sark").name == "flag_sark" && catalog.entry(named: "flag_cq") == nil)
    #expect(e("skin-tone-2").sequence == "\u{1F3FC}")
    // aliases Discord dropped
    for gone in ["ok_woman", "ok_woman_tone1", "white_flag", "waving_black_flag", "chile", "saudi", "ar", "af"] {
        #expect(catalog.entry(named: gone) == nil, "\(gone)")
    }
    #expect(e("flag_ar").sequence == "\u{1F1E6}\u{1F1F7}")
    // components Discord does not list
    #expect(catalog.entry(named: "red_haired") == nil && catalog.entry(named: "digit_one") == nil)
}

@Test func emoji17IsAbsent() {
    for n in ["distorted_face", "orca", "trombone", "treasure_chest", "ballet_dancer"] {
        #expect(catalog.entry(named: n) == nil, "\(n)")
    }
    // Emoji 16 stays
    for n in ["face_with_bags_under_eyes", "fingerprint", "leafless_tree", "root_vegetable", "splatter", "harp", "shovel", "flag_sark"] {
        #expect(catalog.entry(named: n) != nil, "\(n)")
    }
    // U+1FAEA (distorted face, Emoji 17) is not an emoji to the catalog: text, like Discord
    #expect(catalog.match(in: "\u{1FAEA}", at: "\u{1FAEA}".startIndex) == nil)
}

@Test func categoryAndInCategoryOrder() {
    let first10: [EmojiCategory: [String]] = [
        .people: ["grinning", "smiley", "smile", "grin", "laughing", "face_holding_back_tears", "sweat_smile", "joy", "rofl", "smiling_face_with_tear"],
        .nature: ["dog", "cat", "mouse", "hamster", "rabbit", "fox", "bear", "panda_face", "polar_bear", "koala"],
        .food: ["green_apple", "apple", "pear", "tangerine", "lemon", "lime", "banana", "watermelon", "grapes", "strawberry"],
        .activity: ["soccer", "basketball", "football", "baseball", "softball", "tennis", "volleyball", "rugby_football", "flying_disc", "8ball"],
        .travel: ["red_car", "taxi", "blue_car", "pickup_truck", "minibus", "bus", "trolleybus", "race_car", "police_car", "ambulance"],
        .objects: ["watch", "mobile_phone", "calling", "computer", "keyboard", "desktop", "printer", "mouse_three_button", "trackball", "joystick"],
        .symbols: ["pink_heart", "heart", "orange_heart", "yellow_heart", "green_heart", "light_blue_heart", "blue_heart", "purple_heart", "black_heart", "grey_heart"],
        .flags: ["flag_white", "flag_black", "pirate_flag", "checkered_flag", "triangular_flag_on_post", "rainbow_flag", "transgender_flag", "united_nations", "flag_af", "flag_ax"],
    ]
    #expect(EmojiCategory.allCases == [.people, .nature, .food, .activity, .travel, .objects, .symbols, .flags])
    for (c, names) in first10 {
        #expect(catalog.entries(in: c).prefix(10).map(\.name) == names, "\(c)")
    }
    // positions run 0..<n in each category; regional indicators close the symbols
    for c in EmojiCategory.allCases {
        #expect(catalog.entries(in: c).map(\.position) == Array(0..<catalog.entries(in: c).count))
    }
    #expect(catalog.entries(in: .symbols).suffix(26).allSatisfy { $0.name.hasPrefix("regional_indicator_") })
    // handshake sits before heart_hands, as in Discord's picker
    let people = catalog.entries(in: .people).map(\.name)
    #expect(people.firstIndex(of: "handshake")! + 1 == people.firstIndex(of: "heart_hands")!)
}

@Test func toneMapping() throws {
    let up = e("thumbsup")
    #expect(catalog.variants(of: up).count == 5)
    #expect(catalog.variant(of: up, tone: 1)?.sequence == "\u{1F44D}\u{1F3FB}")  // tone 1 = U+1F3FB
    #expect(catalog.variant(of: up, tone: 5)?.sequence == "\u{1F44D}\u{1F3FF}")
    #expect(catalog.variant(of: up, tone: 3)?.names.contains("thumbsup_tone3") == true)
    #expect(catalog.variant(of: up, tone: 3)?.parent == up.sequence)
    #expect(catalog.variant(of: up, tone: 3)?.category == nil)
    // two-person emoji: 25 children keyed by two tones
    let hs = e("handshake")
    #expect(catalog.variants(of: hs).count == 25)
    let two = try #require(catalog.variant(of: hs, tones: [1, 2]))
    #expect(two.sequence.unicodeScalars.map(\.value).filter { (0x1F3FB...0x1F3FF).contains($0) } == [0x1F3FB, 0x1F3FC])
    // a base without tones has no variants
    #expect(catalog.variants(of: e("joy")).isEmpty)
    // every child sequence ends up where its tones say
    for x in catalog.entries where !x.tones.isEmpty {
        var mods = x.sequence.unicodeScalars.map(\.value).filter { (0x1F3FB...0x1F3FF).contains($0) }.map { Int($0) - 0x1F3FB + 1 }
        if mods.count == 2, mods[0] == mods[1] { mods.removeLast() }  // kiss_man_man_tone3: one tone for both
        #expect(mods == x.tones, "\(x.name)")
    }
}

@Test func lookupResolvesSequencesAsOneEntry() throws {
    func m(_ s: String, at offset: Int = 0) -> EmojiMatch? {
        catalog.match(in: s, at: s.index(s.startIndex, offsetBy: offset))
    }
    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}"
    let f = try #require(m(family + " tail"))
    #expect(f.entry.name == "family_mwgb")
    #expect(String(("\(family) tail")[f.range]) == family)
    // a keycap, with and without FE0F
    let k = try #require(m("1\u{FE0F}\u{20E3}x"))
    let keycap = "1\u{FE0F}\u{20E3}x"
    #expect(k.entry.name == "one")
    #expect(String(keycap[k.range]) == "1\u{FE0F}\u{20E3}")
    #expect(m("1\u{20E3}")?.entry.name == "one")
    #expect(m("1") == nil && m("#") == nil && m("a") == nil)
    // a toned emoji beats its base; text before it is skipped by index
    let t = try #require(m("a\u{1F44D}\u{1F3FB}b", at: 1))
    #expect(t.entry.name == "thumbsup_tone1")
    // FE0F is optional and consumed when present
    #expect(m("\u{2764}")?.entry.name == "heart")
    let h = try #require(m("\u{2764}\u{FE0F}!"))
    #expect(h.entry.name == "heart")
    #expect(String("\u{2764}\u{FE0F}!"[h.range]) == "\u{2764}\u{FE0F}")
    // a flag is one entry, two flags are two
    #expect(m("\u{1F1EF}\u{1F1F5}\u{1F1E6}\u{1F1F7}")?.entry.name == "flag_jp")
    // heart on fire: ZWJ keeps FE0F
    #expect(m("\u{2764}\u{FE0F}\u{200D}\u{1F525}")?.entry.name == "heart_on_fire")
    // (c) text-only
    #expect(m("\u{00A9}\u{FE0F}")?.entry.isTextOnly == true)
    #expect(m("\u{1F600}")?.entry.isTextOnly == false)
}

@Test func searchSamples() {
    func top(_ q: String) -> [String] { catalog.search(q, limit: 6).map(\.name) }
    #expect(top("thumbsup").first == "thumbsup")
    #expect(top("+1").first == "thumbsup")
    #expect(top(":joy:").first == "joy")
    #expect(top("joy").contains("joy_cat"))
    #expect(top("heart").first == "heart")  // exact name first
    #expect(top("heart").contains("heart_eyes"))
    // prefix before contains: "heart_eyes" (prefix) precedes "yellow_heart" (contains)
    let all = catalog.search("heart").map(\.name)
    #expect(all.firstIndex(of: "heart_eyes")! < all.firstIndex(of: "yellow_heart")!)
    // aliases search too
    #expect(catalog.search("thumbs_up").first?.name == "thumbsup")
    #expect(catalog.search("zzzzzz").isEmpty && catalog.search("").isEmpty)
    // base emoji only: no tone children in results
    #expect(catalog.search("thumbsup").allSatisfy { $0.category != nil })
}

@MainActor @Test func cacheEvictsUnderTheByteLimit() throws {
    let svg = try #require(EmojiImageProvider.bundledFolder)
    let names = ["1f600", "2764", "1f1ef-1f1f5", "1f44d"]
    let sizes = try names.map { try Data(contentsOf: svg.appendingPathComponent($0 + ".svg")).count }
    // room for the last two loaded files plus a little, never all four
    let limit = sizes[2] + sizes[3] + 10
    let p = EmojiImageProvider(byteLimit: limit)
    for n in names { #expect(p.image(named: n) != nil) }
    #expect(p.cachedBytes <= limit)
    #expect(p.cachedCount == 2)
    // the oldest two were evicted: loading again works (reload), the newest stay cached
    let again = p.image(named: "1f44d")
    #expect(again === p.image(named: "1f44d"))
    // LRU: touching an old entry keeps it
    let q = EmojiImageProvider(byteLimit: sizes[0] + sizes[1] + 10)
    _ = q.image(named: "1f600"); _ = q.image(named: "2764"); _ = q.image(named: "1f600")  // touch
    _ = q.image(named: "1f1ef-1f1f5")  // evicts 2764, not 1f600
    #expect(q.image(named: "1f600") != nil)
    #expect(q.cachedBytes <= sizes[0] + sizes[1] + 10 || q.cachedCount == 1)
    #expect(p.image(named: "does-not-exist") == nil)
}

// MARK: Rendering. NSImage's SVG support against references rasterised by rsvg-convert 2.62.3

private func rgba(_ image: NSImage, pixels n: Int) -> [UInt8] {
    let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: n, pixelsHigh: n, bitsPerSample: 8, samplesPerPixel: 4,
        hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: n * 4, bitsPerPixel: 32)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    NSGraphicsContext.current?.imageInterpolation = .high
    image.draw(in: NSRect(x: 0, y: 0, width: n, height: n), from: .zero, operation: .copy, fraction: 1)
    NSGraphicsContext.restoreGraphicsState()
    return Array(UnsafeBufferPointer(start: rep.bitmapData!, count: n * n * 4))
}

/// Mean absolute difference over premultiplied RGBA, in 0...255.
private func meanDiff(_ a: [UInt8], _ b: [UInt8]) -> Double {
    func pm(_ v: [UInt8]) -> [Double] {
        stride(from: 0, to: v.count, by: 4).flatMap { i -> [Double] in
            let al = Double(v[i + 3]) / 255
            return [Double(v[i]) * al, Double(v[i + 1]) * al, Double(v[i + 2]) * al, Double(v[i + 3])]
        }
    }
    let x = pm(a), y = pm(b)
    return zip(x, y).reduce(0) { $0 + abs($1.0 - $1.1) } / Double(x.count)
}

@MainActor @Test func svgsRenderLikeTheReferenceAtThreeSizes() throws {
    let refs = try #require(Bundle.module.resourceURL).appendingPathComponent("EmojiReference")
    let provider = EmojiImageProvider()
    for name in ["1f600", "2764", "1f1ef-1f1f5"] {
        let image = try #require(provider.image(named: name), "\(name) did not load")
        for n in [18, 22, 48] {
            let ref = try #require(NSImage(contentsOf: refs.appendingPathComponent("\(name)-\(n).png")))
            let got = rgba(image, pixels: n), want = rgba(ref, pixels: n)
            let d = meanDiff(got, want)
            // not blank: the picture covers a real share of the square
            let covered = stride(from: 3, to: got.count, by: 4).filter { got[$0] > 128 }.count
            #expect(covered > n * n / 4, "\(name)@\(n) is mostly empty (\(covered) px)")
            #expect(d < 3, "\(name)@\(n) mean diff \(d)")
        }
    }
}
