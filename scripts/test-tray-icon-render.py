"""Render the actual Rust hidden frame with the tray-icon 0.25 Win32 conversion.
No desktop capture or UI input: all drawing happens in an off-screen DIB.
"""
import argparse,ctypes as c,json
from pathlib import Path
parser=argparse.ArgumentParser()
parser.add_argument('--frames',type=Path,required=True)
args=parser.parse_args()
user=c.WinDLL('user32',use_last_error=True);gdi=c.WinDLL('gdi32',use_last_error=True)
class Header(c.Structure):
    _fields_=[('size',c.c_uint32),('width',c.c_int32),('height',c.c_int32),('planes',c.c_uint16),('bits',c.c_uint16),('compression',c.c_uint32),('image_size',c.c_uint32),('xppm',c.c_int32),('yppm',c.c_int32),('used',c.c_uint32),('important',c.c_uint32)]
user.CreateIcon.argtypes=[c.c_void_p,c.c_int,c.c_int,c.c_ubyte,c.c_ubyte,c.c_void_p,c.c_void_p];user.CreateIcon.restype=c.c_void_p
user.DestroyIcon.argtypes=[c.c_void_p]
user.DrawIconEx.argtypes=[c.c_void_p,c.c_int,c.c_int,c.c_void_p,c.c_int,c.c_int,c.c_uint,c.c_void_p,c.c_uint]
gdi.CreateCompatibleDC.argtypes=[c.c_void_p];gdi.CreateCompatibleDC.restype=c.c_void_p
gdi.CreateDIBSection.argtypes=[c.c_void_p,c.c_void_p,c.c_uint,c.POINTER(c.c_void_p),c.c_void_p,c.c_uint];gdi.CreateDIBSection.restype=c.c_void_p
gdi.SelectObject.argtypes=[c.c_void_p,c.c_void_p];gdi.SelectObject.restype=c.c_void_p
gdi.DeleteObject.argtypes=[c.c_void_p];gdi.DeleteDC.argtypes=[c.c_void_p]
def render(rgba,background):
    size=32
    # Match tray-icon's CreateIcon path, including its legacy AND mask.
    mask=bytes((rgba[i]-255)&255 for i in range(3,len(rgba),4))
    bgra=bytearray(rgba)
    for i in range(0,len(bgra),4): bgra[i],bgra[i+2]=bgra[i+2],bgra[i]
    and_buffer=c.create_string_buffer(mask);xor_buffer=c.create_string_buffer(bytes(bgra))
    icon=user.CreateIcon(None,size,size,1,32,and_buffer,xor_buffer)
    assert icon,'CreateIcon failed'
    dc=gdi.CreateCompatibleDC(None);bits=c.c_void_p()
    header=Header(c.sizeof(Header),size,-size,1,32,0,size*size*4,0,0,0,0)
    bitmap=gdi.CreateDIBSection(dc,c.byref(header),0,c.byref(bits),None,0)
    assert bitmap and bits.value,'CreateDIBSection failed'
    old=gdi.SelectObject(dc,bitmap)
    try:
        baseline=bytes(background)*size*size
        c.memmove(bits,baseline,len(baseline))
        assert user.DrawIconEx(dc,0,0,icon,size,size,0,None,3),'DrawIconEx failed'
        gdi.GdiFlush()
        result=c.string_at(bits,len(baseline))
        deltas=[max(abs(result[i+j]-baseline[i+j]) for j in range(3)) for i in range(0,len(result),4)]
        return {'max_channel_delta':max(deltas),'changed_pixels':sum(d>0 for d in deltas)}
    finally:
        gdi.SelectObject(dc,old);gdi.DeleteObject(bitmap);gdi.DeleteDC(dc);user.DestroyIcon(icon)
frame=(args.frames/'hidden.rgba').read_bytes()
assert len(frame)==32*32*4
results=[]
for background in [(240,220,200,255),(40,60,80,255)]:
    control=render(bytes(len(frame)),background)
    fixed=render(frame,background)
    assert control['max_channel_delta']>20 and control['changed_pixels']>100,control
    assert fixed['max_channel_delta']<=1 and fixed['changed_pixels']<=1,fixed
    results.append({'background_bgra':background,'old_all_zero_alpha':control,'fixed_hidden_frame':fixed})
report={'passed':True,'real_win32_CreateIcon_DrawIconEx':True,'actual_rust_frame':str(args.frames/'hidden.rgba'),'results':results,'desktop_modified':False}
Path('docs/logs/tray-hidden-frame-native.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
print(json.dumps(report,indent=2))
