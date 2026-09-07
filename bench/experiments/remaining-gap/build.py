from pathlib import Path
import io, json, os, shutil, subprocess, tarfile, time, traceback
root=Path('/workspaces/markdownlint-performance/ab-2026-09-07')
root.mkdir(exist_ok=True)
source=root/'source'; source.mkdir(exist_ok=True)
bins=root/'bin'; bins.mkdir(exist_ok=True)
status=root/'build-status.json'
def state(stage,**extra):
 status.write_text(json.dumps({'stage':stage,**extra},indent=2))
 print(stage,extra,flush=True)
def build(name,lto=False,jemalloc=False):
 state('building',variant=name)
 env=os.environ.copy()
 env.pop('CARGO_PROFILE_RELEASE_LTO',None)
 if lto:env['CARGO_PROFILE_RELEASE_LTO']='thin'
 cmd=['cargo','build','--release','--manifest-path',str(source/'Cargo.toml'),'--target-dir','/workspaces/rust-markdownlint/target']
 if jemalloc:cmd+=['--features','rust-markdownlint-cli/perf-jemalloc']
 t=time.monotonic()
 with (root/(name+'-build.log')).open('w') as log:
  subprocess.run(cmd,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
 shutil.copy2('/workspaces/rust-markdownlint/target/release/rust-markdownlint',bins/name)
 print('built',name,time.monotonic()-t,flush=True)
try:
 archive=subprocess.check_output(['git','-C','/workspaces/rust-markdownlint','archive','8bdd65342cd02a11f5e09d02186be51e0e4cc3c6'])
 with tarfile.open(fileobj=io.BytesIO(archive)) as t:t.extractall(source,filter='data')
 p=source/'crates/cli/Cargo.toml';s=p.read_text().replace('[features]\n','[features]\nperf-jemalloc = ["dep:tikv-jemallocator"]\n').replace('[dependencies]\n','[dependencies]\ntikv-jemallocator = { version = "0.6", optional = true }\n');p.write_text(s)
 p=source/'crates/cli/src/main.rs';p.write_text('#[cfg(feature = "perf-jemalloc")]\n#[global_allocator]\nstatic PERF_ALLOCATOR: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;\n\n'+p.read_text())
 build('baseline')
 build('jemalloc',jemalloc=True)
 build('thin-lto',lto=True)
 build('both',lto=True,jemalloc=True)
 state('variants-ready')
 subprocess.run(['python3','/tmp/instrument-markdownlint-ab.py',str(source)],check=True)
 build('profile')
 state('complete')
except BaseException as e:
 state('failed',error=str(e));traceback.print_exc();raise
