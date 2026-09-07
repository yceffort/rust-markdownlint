from pathlib import Path
import json,os,shutil,statistics,subprocess,time,traceback
root=Path('/workspaces/markdownlint-performance/ab-2026-09-07');bins=root/'bin'
status=root/'allocator-profile-status.json'
def state(stage,**extra):
 status.write_text(json.dumps({'stage':stage,**extra}));print(stage,extra,flush=True)
try:
 state('waiting-for-measurements')
 for _ in range(240):
  s=json.loads((root/'measure-status.json').read_text())
  if s['stage']=='failed':raise RuntimeError(s)
  if s['stage']=='complete':break
  time.sleep(5)
 else:raise RuntimeError('wait timeout')
 state('building-profile-jemalloc')
 env=os.environ.copy();env.pop('CARGO_PROFILE_RELEASE_LTO',None)
 with (root/'profile-jemalloc-build.log').open('w') as log:
  subprocess.run(['cargo','build','--release','--locked','--manifest-path',str(root/'source/Cargo.toml'),'--target-dir','/workspaces/rust-markdownlint/target','--features','rust-markdownlint-cli/perf-jemalloc'],env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
 shutil.copy2('/workspaces/rust-markdownlint/target/release/rust-markdownlint',bins/'profile-jemalloc')
 corpus=root/'corpora/blog'
 report=json.loads((root/'results.json').read_text());report['profiles_allocator_comparison']={}
 for mode,threads,count in [('parallel',None,10),('single-thread',1,5)]:
  env=os.environ.copy();env['MARKDOWNLINT_PERF_OUT']=str(root/'allocator-phase-current.json')
  if threads:env['RAYON_NUM_THREADS']=str(threads)
  else:env.pop('RAYON_NUM_THREADS',None)
  cmds={'system':[str(bins/'profile'),'**/*.md'],'jemalloc':[str(bins/'profile-jemalloc'),'**/*.md']}
  outputs={k:subprocess.run(c,cwd=corpus,env=env,capture_output=True) for k,c in cmds.items()}
  a,b=outputs.values();assert a.returncode==b.returncode==1 and (a.stdout,a.stderr)==(b.stdout,b.stderr),'profile allocator output differs'
  samples={k:[] for k in cmds}
  for i in range(count+2):
   order=list(cmds) if i%2==0 else list(reversed(cmds))
   for name in order:
    t=time.perf_counter()
    r=subprocess.run(cmds[name],cwd=corpus,env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    wall=(time.perf_counter()-t)*1000
    assert r.returncode==1
    metrics=json.loads((root/'allocator-phase-current.json').read_text())
    metrics['external_wall_ms']=wall;metrics['adapter_derived_ms']=metrics['parse_tree']-metrics['parser_engine']
    if i>=2:samples[name].append(metrics)
   state('profiling',mode=mode,round=i+1)
  report['profiles_allocator_comparison'][mode]={'matches_system_output':True,'samples_ms':samples,'mean_ms':{name:{key:statistics.mean(x[key] for x in values) for key in values[0]} for name,values in samples.items()}}
  (root/'results.json').write_text(json.dumps(report,indent=2)+'\n')
 # Restore the original workspace's normal release binary after experiments.
 state('restoring-workspace-build')
 with (root/'restore-build.log').open('w') as log:
  subprocess.run(['cargo','build','--release','--locked','--manifest-path','/workspaces/rust-markdownlint/Cargo.toml'],stdout=log,stderr=subprocess.STDOUT,check=True)
 state('complete')
except BaseException as e:
 state('failed',error=str(e));traceback.print_exc();raise
