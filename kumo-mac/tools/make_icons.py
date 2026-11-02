"""Builds assets/kumo-icons.ttf, the icon font of the control panel on a Mac.

The panel asks for glyphs by their Segoe Fluent Icons code point. That font only exists on Windows
and may not be shipped, so this script takes the matching glyphs from Fluent UI System Icons (MIT,
github.com/microsoft/fluentui-system-icons) and puts them on the same code points.

    python make_icons.py FluentSystemIcons-Regular.ttf FluentSystemIcons-Regular.json out.ttf
"""
import json
import sys

from fontTools import subset
from fontTools.ttLib import TTFont

GLYPHS = {
    0xE7F4: "desktop",
    0xE74D: "delete",
    0xE73E: "checkmark",
    0xEC4A: "top_speed",
    0xE769: "pause",
    0xE946: "info",
    0xE91B: "image",
    0xE8B7: "folder",
    0xE8A9: "grid",
    0xE838: "folder_open",
    0xE7E8: "power",
    0xE783: "error_circle",
    0xE774: "globe",
    0xE771: "paint_brush",
    0xE768: "play",
    0xE753: "cloud",
    0xE740: "full_screen_maximize",
    0xE714: "video",
    0xE710: "add",
    0xEB51: "heart",
    0xE9D9: "document_text",
    0xE962: "layer",
    0xE950: "cloud",
    0xE943: "code",
    0xE8F1: "library",
    0xE8BE: "leaf_two",
    0xE8A5: "document",
    0xE896: "arrow_download",
    0xE863: "battery_saver",
    0xE855: "battery_7",
    0xE823: "clock",
    0xE7BA: "warning",
    0xE7AD: "arrow_rotate_clockwise",
    0xE790: "color",
    0xE777: "arrow_reset",
    0xE76C: "chevron_right",
    0xE72E: "lock_closed",
    0xE72C: "arrow_clockwise",
    0xE72B: "arrow_left",
    0xE713: "settings",
    0xE712: "more_horizontal",
    0xE711: "dismiss",
}


def main(font_path, map_path, out_path):
    names = json.load(open(map_path, encoding="utf-8"))
    font = TTFont(font_path)
    source = font.getBestCmap()
    wanted = {}
    for code, name in GLYPHS.items():
        key = f"ic_fluent_{name}_20_regular"
        if key not in names:
            sys.exit(f"missing icon: {key}")
        wanted[code] = source[names[key]]

    options = subset.Options()
    options.layout_features = []
    options.name_IDs = ["*"]
    options.notdef_outline = True
    options.glyph_names = False
    options.hinting = False
    subsetter = subset.Subsetter(options)
    subsetter.populate(glyphs=sorted(set(wanted.values())))
    subsetter.subset(font)

    font["cmap"].tables = [t for t in font["cmap"].tables if t.format == 4][:1] or font["cmap"].tables[:1]
    font["cmap"].tables[0].cmap = dict(wanted)

    family = "Kumo Icons"
    names_table = font["name"]
    names_table.names = [n for n in names_table.names if n.nameID in (0, 13, 14)]
    for name_id, value in [(1, family), (2, "Regular"), (3, family), (4, family), (6, "KumoIcons-Regular")]:
        names_table.setName(value, name_id, 3, 1, 0x409)
    font.save(out_path)
    print(len(GLYPHS), "icons,", out_path)


if __name__ == "__main__":
    main(*sys.argv[1:4])
