#!/usr/bin/env python3
"""Exercise the actual dlopen ABI; run with the package's library/cache paths."""
import base64
import ctypes as c
import pathlib
import tempfile

sqlite = c.CDLL("libsqlite3.so.0")
sqlite.sqlite3_open.argtypes = [c.c_char_p, c.POINTER(c.c_void_p)]
sqlite.sqlite3_exec.argtypes = [c.c_void_p, c.c_char_p, c.c_void_p, c.c_void_p, c.c_void_p]
sqlite.sqlite3_prepare_v2.argtypes = [c.c_void_p, c.c_char_p, c.c_int, c.POINTER(c.c_void_p), c.c_void_p]
sqlite.sqlite3_step.argtypes = [c.c_void_p]
sqlite.sqlite3_column_int.argtypes = [c.c_void_p, c.c_int]
sqlite.sqlite3_finalize.argtypes = [c.c_void_p]
sqlite.sqlite3_close.argtypes = [c.c_void_p]
db, statement = c.c_void_p(), c.c_void_p()
assert sqlite.sqlite3_open(b":memory:", c.byref(db)) == 0
try:
    assert sqlite.sqlite3_exec(db, b"CREATE TABLE clips (id INTEGER); INSERT INTO clips VALUES (42);", None, None, None) == 0
    assert sqlite.sqlite3_prepare_v2(db, b"SELECT id FROM clips", -1, c.byref(statement), None) == 0
    try:
        assert sqlite.sqlite3_step(statement) == 100
        assert sqlite.sqlite3_column_int(statement, 0) == 42
    finally:
        sqlite.sqlite3_finalize(statement)
finally:
    sqlite.sqlite3_close(db)
print("PASS: dynamically loaded SQLite database create/insert/read")

pixbuf = c.CDLL("libgdk_pixbuf-2.0.so.0")
pixbuf.gdk_pixbuf_new_from_file_at_scale.argtypes = [c.c_char_p, c.c_int, c.c_int, c.c_int, c.c_void_p]
pixbuf.gdk_pixbuf_new_from_file_at_scale.restype = c.c_void_p
pixbuf.gdk_pixbuf_get_width.argtypes = [c.c_void_p]
pixbuf.g_object_unref.argtypes = [c.c_void_p]
fixtures = {
    "gif": "R0lGODdhAgACAIEAAEZQWgAAAAAAAAAAACwAAAAAAgACAAAIBgABCAQQEAA7",
    "webp": "UklGRi4AAABXRUJQVlA4ICIAAABwAQCdASoCAAIAAUAmJZQCdAFAAAD+8qlC1ElS8n6IwAAA",
}
with tempfile.TemporaryDirectory(prefix="clipvault-codec-test-") as temp:
    for extension, encoded in fixtures.items():
        path = pathlib.Path(temp) / ("fixture." + extension)
        path.write_bytes(base64.b64decode(encoded))
        image = pixbuf.gdk_pixbuf_new_from_file_at_scale(bytes(path), 480, 240, 1, None)
        assert image, f"GdkPixbuf could not load {extension}"
        try:
            assert pixbuf.gdk_pixbuf_get_width(image) > 0
        finally:
            pixbuf.g_object_unref(image)
        print(f"PASS: dynamically loaded GdkPixbuf {extension} thumbnail")
