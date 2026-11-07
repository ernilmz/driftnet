#!/usr/bin/env python3
"""Opt-in public endpoint test. Never launches apps or changes system proxy/DNS."""
from pathlib import Path
import argparse,json,subprocess,shutil,socket,sys,time,os
root=Path(__file__).resolve().parent.parent
parser=argparse.ArgumentParser();parser.add_argument('--release',action='store_true');args=parser.parse_args()
build=root/'target'/('release' if args.release else 'debug')
host=next(line.split(': ',1)[1] for line in subprocess.check_output(['rustc','-vV'],text=True).splitlines() if line.startswith('host: '))
suffix='.exe' if os.name=='nt' else ''
helper=root/'apps/desktop/binaries'/('drift-engine-'+host+suffix)
probe=root/'target/network-probe';probe.mkdir(exist_ok=True)
agent=probe/('drift-agent'+suffix)
shutil.copy2(build/('drift-agent'+suffix),agent);shutil.copy2(helper,probe/('drift-engine'+suffix))
config={'schema_version':1,'rules':[
{'id':'00000000-0000-0000-0000-000000000001','enabled':True,'target':{'kind':'discord'}},
{'id':'00000000-0000-0000-0000-000000000002','enabled':True,'target':{'kind':'site','domain':'example.com','subdomains':True}}]}
p=subprocess.Popen([str(agent),'--probe'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
try:
    p.stdin.write(json.dumps(config)+'\n');p.stdin.flush()
    reply=json.loads(p.stdout.readline());assert reply['ok'],reply
    def request(url,port):
        result=subprocess.run(['curl','--silent','--show-error','--max-time','20','--noproxy','','--proxy',f'http://127.0.0.1:{port}','--output',__import__('os').devnull,'--write-out','%{http_code} %{http_connect}',url],capture_output=True,text=True)
        return result.stdout.strip().split(),result.returncode
    for url in ['https://discord.com/api/v10/gateway','https://cdn.discordapp.com/embed/avatars/0.png','https://discord.com/api/updates/stable?platform=osx']:
        code,result=request(url,reply['app_port']);print(url.split('?')[0],code);assert result==0 and code==['200','200']
    code,result=request('https://discord.com/download',reply['site_port']);print('Discord website without desktop app launch',code);assert code==['200','200'] and result==0
    download=subprocess.run(['curl','--silent','--show-error','--location','--max-time','30','--max-filesize','2097152','--range','0-65535','--noproxy','','--proxy',f"http://127.0.0.1:{reply['site_port']}",'--output',os.devnull,'--write-out','%{http_code} %{size_download} %{url_effective}','https://discord.com/api/downloads/distributions/app/installers/latest?arch=x64&channel=stable&platform=win'],capture_output=True,text=True)
    details=download.stdout.split();print('Windows Discord download redirect + partial transfer',details);assert download.returncode==0 and details[0]=='206' and int(details[1])==65536
    code,result=request('https://example.com',reply['site_port']);print('selected site',code);assert code==['200','200'] and result==0
    code,result=request('https://github.com',reply['site_port']);print('unselected site refused by site broker',code,result);assert result!=0 and code[-1]=='403'
    pac=subprocess.check_output(['curl','--silent','--show-error','--noproxy','*',reply['pac_url']],text=True)
    script=pac+"\nconsole.log(JSON.stringify(['example.com','sub.example.com','example.org','example.com.evil.org'].map(h=>FindProxyForURL('https://'+h,h))));"
    routes=json.loads(subprocess.check_output(['node','-e',script],text=True));assert routes[0].startswith('PROXY ') and routes[1].startswith('PROXY ') and routes[2:]==['DIRECT','DIRECT'];print('PAC boundary and DIRECT cases passed')
    p.stdin.close();p.wait(timeout=25);assert p.returncode==0
    for port in [reply['app_port'],reply['site_port']]:
        try: socket.create_connection(('127.0.0.1',port),timeout=.2)
        except OSError: pass
        else: raise AssertionError('listener survived shutdown')
    print('Clean shutdown: local listeners closed. OS settings unchanged throughout.')
finally:
    if p.poll() is None:p.kill();p.wait()

# Owner loss during initial setup must cancel promptly. Probe mode never starts apps or changes OS settings.
started=time.monotonic()
cancelled=subprocess.Popen([str(agent),'--probe'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
cancelled.communicate(json.dumps(config)+'\n',timeout=5)
assert cancelled.returncode==0 and time.monotonic()-started<5
print('Owner EOF during setup: cancelled cleanly within 5 seconds.')

# A generic website-only session has no Discord rule, app lookup or remote
# Discord startup dependency. It still provides selective routing.
site_only={'schema_version':1,'rules':[config['rules'][1]]}
standalone=subprocess.Popen([str(agent),'--probe'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
try:
    standalone.stdin.write(json.dumps(site_only)+'\n');standalone.stdin.flush()
    ready=json.loads(standalone.stdout.readline());assert ready['ok'],ready
    code,result=request('https://example.com',ready['site_port']);assert result==0 and code==['200','200']
    code,result=request('https://discord.com',ready['site_port']);assert result!=0 and code[-1]=='403'
    standalone.stdin.close();standalone.wait(timeout=10);assert standalone.returncode==0
    print('Generic website-only session passed without Discord preset or app.')
finally:
    if standalone.poll() is None:standalone.kill();standalone.wait()
