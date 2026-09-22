#!/usr/bin/env python3
"""
Kindle Scribe Notebook (.nbk) Vector SVG Extractor
Extracts all handwritten pages and pen strokes from Amazon Kindle Scribe KDF/KFX notebooks.
Outputs clean vector SVG files for each page.
"""

import sys
import os
import json
import sqlite3
import tempfile
import logging

# Silence noisy third-party logging messages from stdout/stderr
logging.getLogger().setLevel(logging.CRITICAL)

# Add script directory to sys.path to import local ion modules
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
if SCRIPT_DIR not in sys.path:
    sys.path.insert(0, SCRIPT_DIR)

from ion_binary import IonBinary
from ion_symbol_table import LocalSymbolTable, SymbolTableCatalog
from yj_symbol_catalog import SYSTEM_SYMBOL_TABLE, YJ_SYMBOLS
from ion import unannotated, IonSymbol
from utilities import Deserializer

STROKE_COLORS = {
    0: ("black", 0x000000),
    1: ("gray", 0x3f3f3f),
    2: ("red", 0xff0000),
    3: ("orange", 0xff8800),
    4: ("yellow", 0xffff00),
    5: ("green", 0x00ff88),
    7: ("aqua", 0x00ffff),
    8: ("purple", 0x8800ff),
    9: ("pink", 0xff00ff),
    10: ("blue", 0x0000ff),
}

def decode_stroke_values(data, num_points):
    serial = Deserializer(data)
    signature = serial.extract(2)
    if signature != b"\x01\x01":
        return []
    num_vals = serial.unpack("<I")
    if num_vals != num_points or len(serial) * 2 < num_vals:
        return []

    instrs = []
    while len(instrs) < num_vals:
        b = serial.unpack("B")
        instrs.append(b >> 4)
        instrs.append(b & 0x0f)

    if len(instrs) > num_vals:
        instrs.pop(-1)

    vals = []
    for i in range(num_vals):
        instr = instrs[i]
        n = instr & 3
        if instr & 4:
            increment = n
        else:
            if len(serial) < n:
                break
            if n == 0:
                increment = 0
            elif n == 1:
                increment = serial.unpack("B")
            elif n == 2:
                increment = serial.unpack("<H")
            else:
                increment = serial.unpack("B") + (serial.unpack("<H") << 8)

        if instr & 8:
            increment = -increment

        if i == 0:
            change = 0
            value = increment
        else:
            change += increment
            value += change

        vals.append(value)

    return vals

def unwrap_kdf(data):
    FINGERPRINT_OFFSET = 1024
    FINGERPRINT_RECORD_LEN = 1024
    DATA_RECORD_LEN = 1024
    DATA_RECORD_COUNT = 1024
    FINGERPRINT_SIGNATURE = b"\xfa\x50\x0a\x5f"

    if len(data) < FINGERPRINT_OFFSET + 4 or data[FINGERPRINT_OFFSET:FINGERPRINT_OFFSET+4] != FINGERPRINT_SIGNATURE:
        return data

    data_offset = FINGERPRINT_OFFSET
    while len(data) >= data_offset + FINGERPRINT_RECORD_LEN:
        if data[data_offset:data_offset+4] != FINGERPRINT_SIGNATURE:
            break
        data = data[:data_offset] + data[data_offset + FINGERPRINT_RECORD_LEN:]
        data_offset += DATA_RECORD_LEN * DATA_RECORD_COUNT

    return data

def convert_notebook(nbk_path, out_dir):
    if not os.path.isfile(nbk_path):
        return {"success": False, "error": f"File not found: {nbk_path}", "page_count": 0, "pages": []}

    with open(nbk_path, "rb") as f:
        raw_data = f.read()

    unwrapped = unwrap_kdf(raw_data)

    with tempfile.NamedTemporaryFile(suffix=".db", delete=False) as tf:
        tf.write(unwrapped)
        temp_db_path = tf.name

    try:
        con = sqlite3.connect(temp_db_path)
        cur = con.cursor()

        tables = [r[0] for r in cur.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall()]
        if "fragments" not in tables:
            return {"success": False, "error": "Not a valid Scribe KDF database", "page_count": 0, "pages": []}

        catalog = SymbolTableCatalog()
        catalog.add_shared_symbol_table(SYSTEM_SYMBOL_TABLE)
        catalog.add_shared_symbol_table(YJ_SYMBOLS)
        symtab = LocalSymbolTable(catalog=catalog)
        symtab.import_shared_symbol_table(YJ_SYMBOLS)
        ib = IonBinary(symtab)

        sym_row = cur.execute('SELECT payload_value FROM fragments WHERE id=?', ('$ion_symbol_table',)).fetchone()
        if sym_row:
            sym_data = ib.deserialize_multiple_values(sym_row[0])
            if sym_data:
                symtab.create(unannotated(sym_data[0]))

        def get_frag(fid):
            clean_id = str(unannotated(fid))
            row = cur.execute('SELECT payload_value FROM fragments WHERE id=?', (clean_id,)).fetchone()
            if not row:
                return None
            res = ib.deserialize_multiple_values(row[0])
            return unannotated(res[0]) if res else None

        def collect_strokes(item, visited=None):
            if visited is None:
                visited = set()
            strokes = []
            un = unannotated(item)
            if isinstance(un, (IonSymbol, str)):
                sid = str(unannotated(un))
                if sid in visited:
                    return []
                visited.add(sid)
                frag = get_frag(sid)
                if frag:
                    strokes.extend(collect_strokes(frag, visited))
                return strokes

            if isinstance(un, dict):
                if un.get('nmdl.type') == 'nmdl.stroke':
                    strokes.append(un)
                    return strokes
                if '$176' in un:
                    ref_id = str(unannotated(un['$176']))
                    if ref_id not in visited:
                        visited.add(ref_id)
                        frag = get_frag(ref_id)
                        if frag:
                            strokes.extend(collect_strokes(frag, visited))
                if '$146' in un:
                    for child in un['$146']:
                        strokes.extend(collect_strokes(child, visited))
                if '$141' in un:
                    for child in un['$141']:
                        strokes.extend(collect_strokes(child, visited))
            elif isinstance(un, list):
                for child in un:
                    strokes.extend(collect_strokes(child, visited))
            return strokes

        doc_data = get_frag('document_data')
        reading_order = []
        base_prefix = None
        if doc_data and 'nmdl.template_id' in doc_data:
            tmpl_id = str(unannotated(doc_data['nmdl.template_id']))
            if tmpl_id.startswith('z') and len(tmpl_id) > 2:
                base_prefix = 'c' + tmpl_id[1:-1]

        if doc_data and '$169' in doc_data:
            for group in doc_data['$169']:
                un_group = unannotated(group)
                if '$170' in un_group:
                    for pid in un_group['$170']:
                        spid = str(unannotated(pid))
                        pfrag = get_frag(spid)
                        if pfrag and 'nmdl.canvas_width' in pfrag:
                            if base_prefix and not spid.startswith(base_prefix):
                                continue
                            if not spid.startswith('cWlv') and not spid.startswith('cJQ6'):
                                reading_order.append(spid)

        if not reading_order:
            for r in cur.execute('SELECT id FROM fragments WHERE id LIKE "c%"').fetchall():
                spid = r[0]
                if spid.startswith('cWlv'):
                    continue
                pfrag = get_frag(spid)
                if pfrag and 'nmdl.canvas_width' in pfrag:
                    reading_order.append(spid)

        os.makedirs(out_dir, exist_ok=True)
        pages_meta = []

        for p_num, p_id in enumerate(reading_order, 1):
            p_data = get_frag(p_id)
            if not p_data:
                continue
            width = p_data.get('nmdl.canvas_width', 15624)
            height = p_data.get('nmdl.canvas_height', 20832)
            strokes = collect_strokes(p_data)

            svg_lines = [
                f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" width="{width}" height="{height}">',
                f'<rect width="{width}" height="{height}" fill="#ffffff" />'
            ]

            path_count = 0
            for st in strokes:
                brush = st.get('nmdl.brush_type', 0)
                color_code = st.get('nmdl.color', 0)
                color_hex = STROKE_COLORS.get(color_code, ('black', 0x000000))[1]
                color_str = f"#{color_hex:06x}"
                thickness = round(float(st.get('nmdl.thickness', 25.0)))
                bounds = st.get('nmdl.stroke_bounds', [0, 0, width, height])

                st_points = st.get('nmdl.stroke_points', {})
                num_pts = st_points.get('nmdl.num_points', 0)
                if num_pts > 0 and 'nmdl.position_x' in st_points and 'nmdl.position_y' in st_points:
                    xs = decode_stroke_values(st_points['nmdl.position_x'], num_pts)
                    ys = decode_stroke_values(st_points['nmdl.position_y'], num_pts)
                    path_cmds = []
                    for i in range(min(len(xs), len(ys))):
                        px = xs[i] + bounds[0]
                        py = ys[i] + bounds[1]
                        path_cmds.append(f"{'M' if i == 0 else 'L'} {px} {py}")
                    if path_cmds:
                        svg_lines.append(
                            f'<path d="{" ".join(path_cmds)}" fill="none" stroke="{color_str}" stroke-width="{thickness}" stroke-linecap="round" stroke-linejoin="round" />'
                        )
                        path_count += 1

            svg_lines.append('</svg>')
            svg_content = '\n'.join(svg_lines)
            filename = f"page_{p_num}.svg"
            out_file = os.path.join(out_dir, filename)
            with open(out_file, "w", encoding="utf-8") as f:
                f.write(svg_content)

            pages_meta.append({
                "page_num": p_num,
                "filename": filename,
                "stroke_count": path_count,
                "width": width,
                "height": height
            })

        con.close()
        return {
            "success": True,
            "page_count": len(pages_meta),
            "pages": pages_meta
        }
    except Exception as e:
        return {"success": False, "error": str(e), "page_count": 0, "pages": []}
    finally:
        if os.path.exists(temp_db_path):
            try:
                os.remove(temp_db_path)
            except Exception:
                pass

if __name__ == "__main__":
    if len(sys.argv) < 3:
        print(json.dumps({"success": False, "error": "Usage: convert_notebook.py <input.nbk> <output_dir>"}))
        sys.exit(1)

    nbk_file = sys.argv[1]
    destination_dir = sys.argv[2]
    res = convert_notebook(nbk_file, destination_dir)
    print(json.dumps(res))
    sys.exit(0 if res.get("success") else 1)
