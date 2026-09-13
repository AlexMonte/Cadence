"""Converter contracts: safe data parsing, fractional tuning and raw PCM level."""
import base64
import json
from pathlib import Path
import struct
import tempfile
import unittest

from import_webaudiofont import convert, parse_preset


class PresetImportTests(unittest.TestCase):
    def test_data_comments_strings_and_tuning_arithmetic(self):
        preset = parse_preset("console.log('unused'); var sound={zones:[{originalPitch:4200-140+2,fineTune:-20,ahdsr:true,file:'YWJj//AA',},],};")
        self.assertEqual(preset["zones"][0]["originalPitch"], 4062)
        self.assertEqual(preset["zones"][0]["file"], "YWJj//AA")
        for source in ["var x={zones:loadFile()};", "var x={zones:process.exit(0)};", "var x={zones:[1*2]};"]:
            with self.assertRaises(ValueError):
                parse_preset(source)

    def test_raw_pcm_retains_webaudiofont_half_scale_and_loop_coordinates(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            source = folder / "preset.json"
            source.write_text(json.dumps({"zones": [{"sampleRate":8000,"originalPitch":6000,"fineTune":-25,
                "sample":base64.b64encode(struct.pack("<4h", -32768, 8192, 16384, 32767)).decode(),
                "loopStart":1,"loopEnd":3}]}))
            path = convert(str(source), folder / "instrument", "Fixture")
            member = json.loads(path.read_text())["variants"][0]
            self.assertEqual(member["options"]["root_pitch"], 60.25)
            self.assertEqual(member["options"]["sustain_loop"], {"start_frame":1,"end_frame":3})
            data = (path.parent / member["path"]).read_bytes()
            self.assertEqual(struct.unpack("<4f", data[44:]), (-0.5, 0.125, 0.25, 32767/65536))
            with self.assertRaises(ValueError):
                convert(str(source), folder / "instrument", "Fixture")


if __name__ == "__main__":
    unittest.main()
