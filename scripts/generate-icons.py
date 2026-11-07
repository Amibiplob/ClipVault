#!/usr/bin/env python3
"""Rasterize only assets/clipvault.svg using the system librsvg and Cairo."""
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
handle = rsvg.rsvg_handle_new_from_file(bytes(root/'assets/clipvault.svg'), None)
assert handle, 'Cannot load existing ClipVault logo'
for size in (16,24,32,48,64,128,256,512):
    surface = cairo.cairo_image_surface_create(0,size,size)
    ctx = cairo.cairo_create(surface)
    cairo.cairo_scale(ctx,size/256,size/256)
    assert rsvg.rsvg_handle_render_cairo(handle,ctx)
    path = root/f'assets/icons/{size}x{size}/clipvault.png'
    path.parent.mkdir(parents=True,exist_ok=True)
    assert cairo.cairo_surface_write_to_png(surface,bytes(path)) == 0
    cairo.cairo_destroy(ctx)
    cairo.cairo_surface_destroy(surface)
gobject.g_object_unref(handle)
print('Generated 8 PNG sizes from assets/clipvault.svg')
