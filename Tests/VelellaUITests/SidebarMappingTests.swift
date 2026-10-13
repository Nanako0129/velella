import Testing
@testable import VelellaUI

@Test func initialsTakeTheFirstLetterOfUpToThreeWords() {
    #expect(initials(of: "Lorem Ipsum Club") == "LIC")
    #expect(initials(of: "lorem ipsum club dolor") == "LIC")
    #expect(initials(of: "  spaced   out\tname ") == "SON")
    #expect(initials(of: "Solo") == "S")
    #expect(initials(of: "Unknown server") == "US")
    #expect(initials(of: "") == "")
}

@Test func folderColorIntegersBecomeRGB() {
    #expect(RGB(rgb: 0xFF0000) == RGB(1, 0, 0))
    #expect(RGB(rgb: 0x000080) == RGB(0, 0, 128.0 / 255))
    #expect(RGB.defaultFolder == RGB(rgb: 0x5865F2))
}
