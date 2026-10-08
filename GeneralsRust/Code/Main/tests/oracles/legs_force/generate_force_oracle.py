#!/usr/bin/env python3
"""Pinned original TWO_LEGS force extraction. No Git or Rust build required."""
import argparse,hashlib,json,pathlib,shutil,subprocess
ROOT=pathlib.Path(__file__).resolve().parent
PIN_MANIFEST_SHA256='9019fce4ff9404c8fb3217512c7a90e02b0601167ec00c87da5a249f912d568a'
HEADER='''# Original-extracted TWO_LEGS scalar force oracle; not original engine or Main runtime.
# All floats below are hexadecimal IEEE-754 f32 bits. Last three columns are integers.
# lane name velocity_xyz initial_angle goal_angle supplied_post_angle initial_direction_xyz supplied_post_direction_xyz desired_speed acceleration_or_step braking_or_step mass initial_speed post_rotation_speed requested_angle impulse_xyz integrated_velocity_xyz rotation_calls mass_reads motive_expires
# Original XYZ maps to host (X,Z,-Y). The output uses original XYZ throughout.
# raw-frame: original seconds-to-frame conversions on input velocity/speed/acceleration/braking.
# host-impulse: supplied host-unit velocity; parsed/rebound host speed; rebound acceleration/braking times fixed dt.
# exact-frame: literal frame scalars without conversion.
# Approach explicitly disabled; width zero; rotation/object/template/direction providers are harness stubs.
'''
def sha(b):return hashlib.sha256(b).hexdigest()
def main():
 p=argparse.ArgumentParser();p.add_argument('--source-root',type=pathlib.Path,default=ROOT.parents[5]);p.add_argument('--output',type=pathlib.Path,required=True);p.add_argument('--compiler',default='/usr/bin/c++');a=p.parse_args()
 manifest_bytes=(ROOT/'source-pins.json').read_bytes()
 if sha(manifest_bytes)!=PIN_MANIFEST_SHA256:raise SystemExit('source-pin manifest SHA256 mismatch')
 pins=json.loads(manifest_bytes);sources={}
 for key,info in pins['sources'].items():
  data=(a.source_root/info['path']).read_bytes()
  if sha(data)!=info['sha256']:raise SystemExit('original source SHA256 mismatch: '+info['path'])
  sources[key]=data
 blocks={}
 for name,spec in pins['blocks'].items():
  data=b''.join(sources[spec['source']].splitlines(keepends=True)[spec['first_line']-1:spec['last_line']])
  if sha(data)!=spec['sha256']:raise SystemExit('original block SHA256 mismatch: '+name)
  blocks[name]=data.decode()
 a.output.mkdir(parents=True,exist_ok=True);(a.output/'extracted').mkdir(exist_ok=True)
 for name,value in blocks.items():(a.output/'extracted'/(name+'.inc')).write_bytes(value.encode())
 def original(name):
  spec=pins['blocks'][name];path=pins['sources'][spec['source']]['path']
  return '\n// BEGIN UNCHANGED ORIGINAL '+name+'\n#line '+str(spec['first_line'])+' "'+path+'"\n'+blocks[name]+'\n// END UNCHANGED ORIGINAL '+name+'\n'
 prefix=(ROOT/'harness-prefix.cpp').read_text()
 prefix+=''.join(original(n) for n in ['pi','types','sqr','frames','normalize','angle_diff','slowdown','motive_frames'])
 prefix+='\n#line 1 "harness-shims.cpp"\n'+(ROOT/'harness-shims.cpp').read_text()
 original_bodies=''.join(original(n) for n in ['forward','is_motive','force','motive','legs'])
 original_bodies+='\nvoid PhysicsBehavior::integrate_original_velocity() {\n'+original('velocity_step')+'}\n'
 suffix='\n#line 1 "harness-cases.cpp"\n'+(ROOT/'harness-cases.cpp').read_text()
 # These deliberately faulty variants are diagnostic controls, never original evidence.
 dot=original_bodies.replace('Real speed = (Real)sqrtf( speedSquared );','Real speed = (Real)fabs(dot);')
 assert dot!=original_bodies
 late=original_bodies.replace('locoUpdate_moveTowardsAngle(obj, desiredAngle);','locoUpdate_moveTowardsAngle(obj, desiredAngle);\n\tactualSpeed = physics->getForwardSpeed2D();')
 assert late!=original_bodies
 variants={'original':original_bodies,'mutation-dot-speed':dot,'mutation-late-sample':late}
 compiler=pathlib.Path(shutil.which(a.compiler) or a.compiler).resolve()
 version=subprocess.check_output([str(compiler),'--version'],text=True)
 runs=[]
 for name,bodies in variants.items():
  src=a.output/(name+'.cpp');binary=a.output/name
  if name=='mutation-dot-speed':
   bodies=bodies.replace('UNCHANGED ORIGINAL forward','DIAGNOSTIC MUTATION forward')
  elif name=='mutation-late-sample':
   bodies=bodies.replace('UNCHANGED ORIGINAL legs','DIAGNOSTIC MUTATION legs')
  src.write_text(prefix+bodies+suffix)
  cmd=[str(compiler),'-std=c++17','-O0','-ffp-contract=off','-fno-fast-math',str(src),'-o',str(binary)]
  result=subprocess.run(cmd,capture_output=True,text=True)
  (a.output/(name+'.build.log')).write_text(result.stdout+result.stderr)
  if result.returncode:raise SystemExit(result.stderr)
  raw=subprocess.check_output([str(binary)])
  (a.output/(name+'.txt')).write_bytes(HEADER.encode()+raw)
  runs.append({'name':name,'command':cmd,'source_sha256':sha(src.read_bytes()),'binary_sha256':sha(binary.read_bytes()),'output_sha256':sha((a.output/(name+'.txt')).read_bytes()),'rows':len(raw.splitlines())})
 result={'kind':'original scalar extraction with labelled stubs and diagnostic mutations','source_revision':pins['revision'],'source_pin_manifest_sha256':PIN_MANIFEST_SHA256,'compiler':str(compiler),'compiler_sha256':sha(compiler.read_bytes()),'compiler_version':version,'generator_sha256':sha(pathlib.Path(__file__).read_bytes()),'harness_sha256':{n:sha((ROOT/n).read_bytes()) for n in ['harness-prefix.cpp','harness-shims.cpp','harness-cases.cpp']},'runs':runs}
 (a.output/'provenance.json').write_text(json.dumps(result,indent=2)+'\n')
 print(json.dumps({'rows_per_variant':len(raw.splitlines()),'variants':list(variants),'output':str(a.output)}))
if __name__=='__main__':main()
