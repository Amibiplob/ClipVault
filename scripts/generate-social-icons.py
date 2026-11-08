#!/usr/bin/env python3
"""Rasterize bundled social SVG icons using system librsvg and Cairo."""
import ctypes as c
import pathlib
root = pathlib.Path(__file__).resolve().parents[1]
rsvg = c.CDLL('librsvg-2.so.2')
cairo = c.CDLL('libcairo.so.2')
gobject = c.CDLL('libgobject-2.0.so.0')
rsvg.rsvg_handle_new_from_file.argtypes = [c.c_char_p, c.c_void_p]
rsvg.rsvg_handle_new_from_file.restype = c.c_void_p
rsvg.rsvg_handle_render_cairo.argtypes = [c.c_void_p, c.c_void_p]
rsvg.rsvg_handle_render_cairo.restype = c.c_int
cairo.cairo_image_surface_create.argtypes = [c.c_int,c.c_int,c.c_int]
cairo.cairo_image_surface_create.restype = c.c_void_p
cairo.cairo_create.argtypes = [c.c_void_p]
cairo.cairo_create.restype = c.c_void_p
cairo.cairo_scale.argtypes = [c.c_void_p,c.c_double,c.c_double]
cairo.cairo_surface_write_to_png.argtypes = [c.c_void_p,c.c_char_p]
cairo.cairo_surface_write_to_png.restype = c.c_int
cairo.cairo_destroy.argtypes = [c.c_void_p]
cairo.cairo_surface_destroy.argtypes = [c.c_void_p]
gobject.g_object_unref.argtypes = [c.c_void_p]

for name in ('github', 'linkedin'):
    handle = rsvg.rsvg_handle_new_from_file(bytes(root/f'assets/social/{name}.svg'), None)
    assert handle
    size = 48
    surface = cairo.cairo_image_surface_create(0,size,size)
    ctx = cairo.cairo_create(surface)
    cairo.cairo_scale(ctx,size/16,size/16)
    assert rsvg.rsvg_handle_render_cairo(handle,ctx)
    assert cairo.cairo_surface_write_to_png(surface,bytes(root/f'assets/social/{name}.png')) == 0
    cairo.cairo_destroy(ctx)
    cairo.cairo_surface_destroy(surface)
    gobject.g_object_unref(handle)
print('Generated bundled MIT brand icon rasterizations')
