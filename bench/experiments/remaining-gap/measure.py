from pathlib import Path
import hashlib,json,os,platform,shutil,statistics,subprocess,time,tomllib,traceback
root=Path('/workspaces/markdownlint-performance/ab-2026-09-07')
bins=root/'bin'; corpusroot=root/'corpora';corpusroot.mkdir(exist_ok=True)
statusfile=root/'measure-status.json'
def state(stage,**extra):
 statusfile.write_text(json.dumps({'stage':stage,**extra}));print(stage,extra,flush=True)
def run(cmd,cwd,env=None,capture=False):
 t=time.perf_counter()
 r=subprocess.run(cmd,cwd=cwd,env=env,stdout=subprocess.PIPE if capture else subprocess.DEVNULL,stderr=subprocess.PIPE if capture else subprocess.DEVNULL,timeout=300 if capture else None)
 elapsed=time.perf_counter()-t
 if r.returncode not in [0,1]:raise RuntimeError(f'exit {r.returncode}: {cmd}')
 return r,elapsed
def sha(b):return hashlib.sha256(b).hexdigest()
def output(*cmd):return subprocess.check_output(cmd,text=True).strip()
def summary(values):return {'seconds':values,'mean':statistics.mean(values),'stddev':statistics.stdev(values),'median':statistics.median(values),'min':min(values),'max':max(values)}
try:
 state('waiting-for-builds')
 for _ in range(240):
  status=json.loads((root/'build-status.json').read_text())
  if status['stage']=='failed':raise RuntimeError(status)
  if status['stage']=='complete':break
  time.sleep(10)
 else:raise RuntimeError('build wait timeout')
 tools=['baseline','thin-lto','jemalloc','both','rumdl']
 commands={k:[str(bins/k),'**/*.md'] for k in tools[:-1]}
 commands['rumdl']=['/workspaces/markdownlint-performance/bin/rumdl','check','--no-cache','--no-config','.']
 lock=tomllib.loads((root/'source/Cargo.lock').read_text())
 report={'source_commit':'8bdd65342cd02a11f5e09d02186be51e0e4cc3c6','blog_commit':'4c7cade067a10eb565a8e608081532fa055218c3','date_utc':output('date','-u','+%Y-%m-%dT%H:%M:%SZ'),
 'environment':{'platform':platform.platform(),'lscpu':output('lscpu'),'rustc':output('rustc','--version'),'cargo':output('cargo','--version'),'rumdl':output(commands['rumdl'][0],'--version')},
 'allocator_dependencies':[p for p in lock['package'] if 'jemalloc' in p['name']],
 'variants':{k:{'lto':'thin' if k in ['thin-lto','both'] else 'default','allocator':'jemalloc' if k in ['jemalloc','both'] else 'system','sha256':sha((bins/k).read_bytes()),'bytes':(bins/k).stat().st_size} for k in tools[:-1]},
 'method':{'warmup':3,'runs':20,'order':'rotate five tools each round; reverse every five rounds','timing':'perf_counter around subprocess.run with blocking wait; default stdout/stderr to /dev/null','builds_complete_before_timing':True},'corpora':[],'profiles':{}}
 def save(): (root/'results.json').write_text(json.dumps(report,indent=2)+'\n')
 cases=[('fixtures',root/'source/crates/core/tests/fixtures/markdownlint',1),('blog',Path('/workspaces/markdownlint-performance/blog/apps/blog/posts'),1),('fixtures-10x',root/'source/crates/core/tests/fixtures/markdownlint',10)]
 for name,source,scale in cases:
  state('preparing',corpus=name)
  corpus=corpusroot/name;corpus.mkdir(exist_ok=True)
  files=sorted(source.rglob('*.md')) if name=='blog' else sorted(source.glob('*.md'))
  digest=hashlib.sha256();bytes_count=0
  for n in range(1,scale+1):
   for p in files:
    rel=p.relative_to(source)
    if scale>1:rel=Path(str(n))/rel
    dest=corpus/rel;dest.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,dest)
    content=dest.read_bytes();path=rel.as_posix().encode();bytes_count+=len(content)
    digest.update(len(path).to_bytes(8,'big')+path);digest.update(len(content).to_bytes(8,'big')+content)
  (corpus/'.markdownlint-cli2.jsonc').write_text('{"noBanner":true}\n')
  entry={'name':name,'files':len(files)*scale,'bytes':bytes_count,'sha256':digest.hexdigest(),'validation':{},'results':{k:{'seconds':[]} for k in tools}}
  report['corpora'].append(entry)
  captured={}
  for k,cmd in commands.items():
   r,_=run(cmd,corpus,capture=True);captured[k]=r
   entry['validation'][k]={'exit_code':r.returncode,'stdout_sha256':sha(r.stdout),'stderr_sha256':sha(r.stderr),'stderr_lines':len(r.stderr.splitlines())}
   if k in ['thin-lto','jemalloc','both']:
    same=(r.returncode,r.stdout,r.stderr)==(captured['baseline'].returncode,captured['baseline'].stdout,captured['baseline'].stderr)
    entry['validation'][k]['matches_baseline']=same
    if not same:raise RuntimeError(f'output difference {k} {name}')
  for _ in range(3):
   for cmd in commands.values():run(cmd,corpus)
  for i in range(20):
   order=tools[i%5:]+tools[:i%5]
   if (i//5)%2:order=order[::-1]
   for k in order:
    r,elapsed=run(commands[k],corpus)
    if r.returncode!=captured[k].returncode:raise RuntimeError('timed exit code changed')
    entry['results'][k]['seconds'].append(elapsed)
   state('measuring',corpus=name,round=i+1);save()
  for k,v in entry['results'].items():
   entry['results'][k]=summary(v['seconds'])
   print(name,k,entry['results'][k]['mean']*1000,entry['results'][k]['stddev']*1000,flush=True)
  save()
 # Phase totals are separate from the uninstrumented comparison.
 corpus=corpusroot/'blog'
 for label,threads,count in [('parallel',None,10),('single-thread',1,5)]:
  state('profiling',mode=label)
  env=os.environ.copy();env['MARKDOWNLINT_PERF_OUT']=str(root/'phase-current.json')
  if threads:env['RAYON_NUM_THREADS']=str(threads)
  else:env.pop('RAYON_NUM_THREADS',None)
  cmd=[str(bins/'profile'),'**/*.md']
  base,_=run(commands['baseline'],corpus,capture=True)
  prof,_=run(cmd,corpus,env=env,capture=True)
  assert (base.returncode,base.stdout,base.stderr)==(prof.returncode,prof.stdout,prof.stderr),'profile output differs'
  samples=[]
  for i in range(count+2):
   _,elapsed=run(cmd,corpus,env=env)
   metrics=json.loads((root/'phase-current.json').read_text())
   metrics['external_wall_ms']=elapsed*1000
   metrics['adapter_derived_ms']=metrics['parse_tree']-metrics['parser_engine']
   if i>=2:samples.append(metrics)
  report['profiles'][label]={'matches_baseline':True,'samples_ms':samples,'mean_ms':{k:statistics.mean(v[k] for v in samples) for k in samples[0]},'median_ms':{k:statistics.median(v[k] for v in samples) for k in samples[0]}}
  save()
 # Collect rumdl's built-in profiler output for context; do not time this invocation.
 r,_=run(commands['rumdl']+['--profile'],corpus,capture=True)
 (root/'rumdl-profile.stdout').write_bytes(r.stdout);(root/'rumdl-profile.stderr').write_bytes(r.stderr)
 save();state('complete')
except BaseException as e:
 state('failed',error=str(e));traceback.print_exc();raise
