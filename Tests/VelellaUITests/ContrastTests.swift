import Testing
@testable import VelellaUI

@Test func contrastTextAlwaysReachesAA() {
    // Sweep the generated-avatar hue range and greys: the better of white/black must be >= 4.5.
    var backgrounds = (0..<360).map { RGB(hue: Double($0) / 360, saturation: 0.5, brightness: 0.62) }
    backgrounds += (0...10).map { RGB(Double($0) / 10, Double($0) / 10, Double($0) / 10) }
    for bg in backgrounds {
        #expect(bg.contrastRatio(with: ContrastText.rgb(on: bg)) >= 4.5)
    }
}

@Test func contrastRatioMatchesWCAGReference() {
    #expect(abs(RGB(0, 0, 0).contrastRatio(with: RGB(1, 1, 1)) - 21) < 0.001)
}
