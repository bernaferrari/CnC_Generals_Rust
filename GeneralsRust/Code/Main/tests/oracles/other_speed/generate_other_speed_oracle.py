#!/usr/bin/env python3
"""Pinned OTHER speed extraction. No repository, Git, Cargo, or downloads."""
import argparse, hashlib, json, pathlib, shutil, subprocess
ROOT = pathlib.Path(__file__).resolve().parent
PIN_MANIFEST_SHA256 = 'e5e4fe029f7c61238f8d800e5168c735849fae1bc70420d50a8889e6fe26205e'
HEADER = '''# Original-extracted OTHER force-speed oracle; not original engine or Main runtime.
# Fields: lane name velocity_xyz position_xyz target_xyz entry_angle supplied_post_angle entry_direction_xyz supplied_post_direction_xyz desired_speed acceleration_or_step braking_or_step mass entry_speed exit_speed raw_slide_factor_frames lane_slide_factor path_distance observed_exit_angle observed_exit_direction_xyz impulse_xyz integrated_velocity_xyz adapter_displacement_xyz ultra rotation_calls turning_calls turning mass_reads motive_expires
# Float fields use hexadecimal IEEE-754 f32 bits. Last six fields are integers.
# Original XYZ maps to host (X,Z,-Y). All output vectors remain original XYZ.
# raw-frame uses original seconds-to-frame conversions. host-impulse explicitly adapts parsed/rebound host scalars to one impulse and slide distance threshold.
# Nonzero velocity distinguishes speed metric and sample phase. Every variant preserves original cached force direction and close-slide override.
# Supplied turn directions/angles are harness input, not extracted rotation results. Approach remains enabled with far normal targets; the slide sentinel is rested.
# Body, mass, position and flag providers are labelled stubs. Diagnostic entry/exit speed columns use the selected variant's speed metric.
# adapter_displacement is a diagnostic projection: raw velocity * 1 or host velocity * dt. It is not an original engine position update.
'''
def sha(data):
    return hashlib.sha256(data).hexdigest()

def main():
    p = argparse.ArgumentParser()
    p.add_argument('--source-root', type=pathlib.Path, default=ROOT.parents[5])
    p.add_argument('--output', type=pathlib.Path, required=True)
    p.add_argument('--compiler', default='/usr/bin/c++')
    a = p.parse_args()
    manifest_bytes = (ROOT/'source-pins.json').read_bytes()
    if sha(manifest_bytes) != PIN_MANIFEST_SHA256:
        raise SystemExit('source-pin manifest SHA256 mismatch')
    pins = json.loads(manifest_bytes)
    sources = {}
    for name, spec in pins['sources'].items():
        data = (a.source_root/spec['path']).read_bytes()
        if sha(data) != spec['sha256']:
            raise SystemExit('original source SHA256 mismatch: '+spec['path'])
        sources[name] = data
    blocks = {}
    for name, spec in pins['blocks'].items():
        data = b''.join(sources[spec['source']].splitlines(keepends=True)[spec['first_line']-1:spec['last_line']])
        if sha(data) != spec['sha256']:
            raise SystemExit('original block SHA256 mismatch: '+name)
        blocks[name] = data
    a.output.mkdir(parents=True, exist_ok=False)
    (a.output/'extracted').mkdir()
    for name, data in blocks.items():
        (a.output/'extracted'/(name+'.inc')).write_bytes(data)
    def original(name):
        spec = pins['blocks'][name]
        path = pins['sources'][spec['source']]['path']
        return (f'\n// BEGIN UNCHANGED ORIGINAL {name}\n#line {spec["first_line"]} "{path}"\n'.encode()
                + blocks[name] + f'\n// END UNCHANGED ORIGINAL {name}\n'.encode())
    prefix = (ROOT/'harness-prefix.cpp').read_bytes()
    prefix += b''.join(original(n) for n in ['pi','types','sqr','frames','slowdown','motive_frames','turning_enum'])
    prefix += b'\n// HARNESS Coord3D wrapper; length/normalize bodies unchanged.\nstruct Coord3D { Real x,y,z;\n'
    prefix += original('coord_length') + original('coord_normalize') + b'};\n'
    prefix += b'\n#line 1 "harness-shims.cpp"\n' + (ROOT/'harness-shims.cpp').read_bytes()
    bodies = b''.join(original(n) for n in ['forward','is_motive','force','motive','other'])
    bodies += b'\n// HARNESS wrapper for original velocity statements, no friction or position phase.\nvoid PhysicsBehavior::integrate_original_velocity() {\n'
    bodies += original('velocity_step') + b'}\n'
    suffix = b'\n#line 1 "harness-cases.cpp"\n' + (ROOT/'harness-cases.cpp').read_bytes()
    # Deliberately faulty controls, never passed off as original code/evidence.
    metric_needle = b'Real speed = (Real)sqrtf( speedSquared );'
    assert bodies.count(metric_needle) == 1
    dot = bodies.replace(metric_needle,b'Real speed = (Real)fabs(dot);')
    timing_needle = b'\t\tphysics->setTurning(rotating);'
    assert bodies.count(timing_needle) == 1
    late = bodies.replace(timing_needle,timing_needle+b'\n\t\tactualSpeed = physics->getForwardSpeed2D();')
    both = dot.replace(timing_needle,timing_needle+b'\n\t\tactualSpeed = physics->getForwardSpeed2D();')
    variants = {'original':bodies,'mutation_entry_dot_metric':dot,
                'mutation_late_signed_sample':late,'mutation_late_dot_both':both}
    compiler = pathlib.Path(shutil.which(a.compiler) or a.compiler).resolve()
    version = subprocess.check_output([str(compiler),'--version'],text=True)
    runs = []
    for name, value in variants.items():
        if name in ['mutation_late_signed_sample','mutation_late_dot_both']:
            value = value.replace(b'UNCHANGED ORIGINAL other',b'DIAGNOSTIC MUTATION other')
        if name in ['mutation_entry_dot_metric','mutation_late_dot_both']:
            value = value.replace(b'UNCHANGED ORIGINAL forward',b'DIAGNOSTIC MUTATION forward')
        source, binary = a.output/(name+'.cpp'), a.output/name
        source.write_bytes(prefix+value+suffix)
        cmd = [str(compiler),'-std=c++17','-O0','-ffp-contract=off','-fno-fast-math',str(source),'-o',str(binary)]
        result = subprocess.run(cmd,capture_output=True,text=True)
        (a.output/(name+'.build.log')).write_text(result.stdout+result.stderr)
        if result.returncode:
            raise SystemExit(result.stderr)
        raw = subprocess.check_output([str(binary)])
        (a.output/(name+'.txt')).write_bytes(HEADER.encode()+raw)
        runs.append({'variant':name,'command':cmd,'source_sha256':sha(source.read_bytes()),'binary_sha256':sha(binary.read_bytes()),'output_sha256':sha((a.output/(name+'.txt')).read_bytes()),'rows':len(raw.splitlines())})
    result = {'kind':'original speed extraction with cached entry force direction, supplied rotation, host/frame adapters and orthogonal metric/timing diagnostics',
              'source_checkout_head_at_capture':pins['source_checkout_head_at_capture'],
              'source_pin_manifest_sha256':PIN_MANIFEST_SHA256,'compiler':str(compiler),
              'compiler_sha256':sha(compiler.read_bytes()),'compiler_version':version,
              'generator_sha256':sha(pathlib.Path(__file__).read_bytes()),
              'harness_sha256':{n:sha((ROOT/n).read_bytes()) for n in ['harness-prefix.cpp','harness-shims.cpp','harness-cases.cpp']},'runs':runs}
    (a.output/'provenance.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({'output':str(a.output),'variants':list(variants),'rows_per_variant':len(raw.splitlines())}))
if __name__ == '__main__':
    main()
