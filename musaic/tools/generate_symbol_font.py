"""Generate the first-party UI symbol fallback (requires fontTools).

No third-party outlines: these are the same geometric strokes as the board atlas.
Run from any directory. The committed font is deterministic and needs no build-
time Python dependency in native or web releases.
"""
from pathlib import Path
from math import hypot
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from musical_symbols import SYMBOLS

ROOT = Path(__file__).resolve().parents[1]
SCALE = 1000 / 96

def glyph(lines):
    pen = TTGlyphPen(None)
    for line in lines:
        for (ax, ay), (bx, by) in zip(line, line[1:]):
            dx, dy = bx - ax, by - ay
            length = hypot(dx, dy)
            nx, ny = -dy / length * 2.5, dx / length * 2.5
            # Stroke rectangles overlap with consistent winding at each joint.
            tx, ty = dx / length * 2.5, dy / length * 2.5
            points = [(ax-tx+nx, ay-ty+ny), (bx+tx+nx, by+ty+ny),
                      (bx+tx-nx, by+ty-ny), (ax-tx-nx, ay-ty-ny)]
            pen.moveTo((round(points[0][0]*SCALE), round((76-points[0][1])*SCALE)))
            for x, y in points[1:]:
                pen.lineTo((round(x*SCALE), round((76-y)*SCALE)))
            pen.closePath()
    return pen.glyph()

def build():
    names = {ord(char): f"uni{ord(char):04X}" for char in SYMBOLS}
    font = FontBuilder(1000, isTTF=True)
    order = ['.notdef', 'space', *names.values()]
    font.setupGlyphOrder(order)
    font.setupCharacterMap({32: 'space', **names})
    glyphs = {'.notdef': glyph([[(10,10),(56,10),(56,66),(10,66),(10,10)]]), 'space': glyph([])}
    glyphs.update({names[ord(char)]: glyph(lines) for char, lines in SYMBOLS.items()})
    font.setupGlyf(glyphs)
    font.setupHorizontalMetrics({name: (688, 0) for name in order})
    font.setupHorizontalHeader(ascent=1000, descent=-200)
    font.setupNameTable({'familyName': 'Musaic Symbols', 'styleName': 'Regular',
                        'uniqueFontIdentifier': 'MusaicSymbols-Regular-1',
                        'fullName': 'Musaic Symbols Regular', 'psName': 'MusaicSymbols-Regular',
                        'version': 'Version 1.000', 'licenseDescription': 'MIT OR Apache-2.0; first-party Musaic asset'})
    font.setupOS2(sTypoAscender=1000, sTypoDescender=-200, usWinAscent=1000, usWinDescent=200)
    font.setupPost()
    font.setupMaxp()
    font.font['head'].created = font.font['head'].modified = 2082844800
    font.font.recalcTimestamp = False
    font.save(ROOT / 'assets/fonts/MusaicSymbols-Regular.ttf')

if __name__ == '__main__':
    build()
