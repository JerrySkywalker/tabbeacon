"""Bounded stdio-only real Codex ancestor for the synthetic launcher fixture.
No model, app-server, network listener, credentials, trust, or daily config.
"""
import ctypes as c
from ctypes import wintypes as w
import json,os,subprocess as sp,sys,time,threading,queue
from pathlib import Path
k=c.WinDLL('kernel32',use_last_error=True); k.FreeConsole()
if not k.AllocConsole(): raise c.WinError(c.get_last_error())
k.GetConsoleWindow.restype=w.HWND
u=c.WinDLL('user32'); u.ShowWindow.argtypes=[w.HWND,c.c_int];u.ShowWindow(k.GetConsoleWindow(),0)
k.CreateFileW.restype=w.HANDLE
k.CreateFileW.argtypes=[w.LPCWSTR,w.DWORD,w.DWORD,c.c_void_p,w.DWORD,w.DWORD,w.HANDLE]
k.CloseHandle.argtypes=[w.HANDLE]
h=k.CreateFileW('CONOUT$',0xC0000000,3,None,3,0,None);mode=w.DWORD()
k.GetConsoleMode(w.HANDLE(h),c.byref(mode)); k.SetConsoleMode(w.HANDLE(h),mode.value|4);k.CloseHandle(h)
native,binary,out=sys.argv[1:4]
root=Path(out).parent/(Path(out).name+'-driver');root.mkdir(exist_ok=False)
env={name:os.environ[name] for name in ['SystemRoot','WINDIR','COMSPEC','PATH','PATHEXT','TEMP','TMP','USERPROFILE'] if name in os.environ}
env.update(CODEX_HOME=str(root/'codex-home'),LOCALAPPDATA=str(root/'appdata'),WT_SESSION='tb-v0801-fixture-owned-console')
for name in ['TB_CONSOLE_FIXTURE_SAMPLES','TB_CONSOLE_FIXTURE_COLD_SAMPLES','TB_CONSOLE_FIXTURE_CONCURRENCY','TB_CONSOLE_FIXTURE_SHELL']:
    if name in os.environ: env[name]=os.environ[name]
Path(env['CODEX_HOME']).mkdir();Path(env['LOCALAPPDATA']).mkdir()
p=sp.Popen([native,'exec-server','--listen','stdio'],env=env,cwd=root,stdin=sp.PIPE,stdout=sp.PIPE,stderr=sp.DEVNULL)
messages=queue.Queue()
def read():
    for line in p.stdout:
        try: messages.put(json.loads(line))
        except ValueError: messages.put({'error':'invalid fixture RPC'})
threading.Thread(target=read,daemon=True).start()
def request(identifier,method,params):
    p.stdin.write((json.dumps({'id':identifier,'method':method,'params':params})+'\n').encode());p.stdin.flush()
    deadline=time.monotonic()+15
    while time.monotonic()<deadline:
        m=messages.get(timeout=max(0.01,deadline-time.monotonic()))
        if m.get('id')==identifier:
            if 'error' in m: raise RuntimeError(m['error'])
            return m['result']
    raise RuntimeError('fixture RPC deadline')
try:
    initialized=request(1,'initialize',{'clientName':'tabbeacon-bounded-console-fixture'})
    p.stdin.write(b'{"method":"initialized"}\n');p.stdin.flush()
    childenv=dict(env);childenv['TB_OWNED_NATIVE_PARENT']=str(p.pid)
    request(2,'process/start',{'processId':'tabbeacon-fixture','argv':[sys.executable,str(Path(__file__).with_name('codex-hook-console-fixture.py')),binary,out],
        'cwd':root.as_uri(),'env':childenv,'tty':False,'pipeStdin':False,'arg0':None})
    # Full cold-install/setup + Hook/SessionEnd samples remain owned and bounded.
    deadline=time.monotonic()+900
    while time.monotonic()<deadline:
        r=request(3,'process/read',{'processId':'tabbeacon-fixture','afterSeq':None,'maxBytes':4096,'waitMs':500})
        if r['exited']:
            (root/'driver-receipt.json').write_text(json.dumps({'native_pid':p.pid,'native_identity':native,
                'transport':'bounded stdio exec-server test ancestor only; product remains command Hooks',
                'fixture_exit':r['exitCode'],'provider_delivery':'UNPROVEN_SYNTHETIC_LAUNCHER'},indent=2))
            if r['exitCode']!=0: raise RuntimeError('fixture exited '+str(r['exitCode']))
            break
    else: raise RuntimeError('fixture deadline')
finally:
    p.kill();p.wait(timeout=5);k.FreeConsole()
