#!/usr/bin/env python3
"""Executable component comparison, not a full-game parity claim.

Extracts original GameLogic heap methods verbatim. Only the child-balancing
#if 1 is selected as #if 0 to compile the original pristine indexed branch,
avoiding pointer arithmetic beyond the vector end in the optimized branch.
Both programs execute operations independently; no expected topology fixture.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile


def extract(text, signature):
    start = text.index(signature)
    opening = text.index('{', start)
    depth = 1
    end = opening + 1
    while depth:
        depth += (text[end] == '{') - (text[end] == '}')
        end += 1
    return text[start:end]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, default=Path(__file__).resolve().parents[4])
    args = parser.parse_args()
    root = args.repo.resolve()
    cpp_path = root / 'GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogic.cpp'
    rust_path = root / 'GeneralsRust/Code/GameEngine/Common/src/indexed_heap.rs'
    cpp = cpp_path.read_text()
    rust = rust_path.read_text()
    cxx = shutil.which('clang++') or shutil.which('g++')
    rustc = shutil.which('rustc')
    if not cxx or not rustc:
        raise RuntimeError('Both a C++ compiler and rustc are required')
    methods = ['eraseSleepyUpdate', 'rebalanceParentSleepyUpdate',
               'rebalanceChildSleepyUpdate', 'rebalanceSleepyUpdate',
               'remakeSleepyUpdate', 'pushSleepyUpdate', 'popSleepyUpdate']
    signatures = ['void', 'Int', 'Int', 'void', 'void', 'void', 'void']
    bodies = []
    for result, name in zip(signatures, methods):
        body = extract(cpp, f'{result} GameLogic::{name}(')
        if name == 'rebalanceChildSleepyUpdate':
            if body.count('#if 1') != 1:
                raise RuntimeError('Original child branch marker changed')
            body = body.replace('#if 1', '#if 0', 1)
        bodies.append(body)
    cpp_driver = r'''
#include <vector>
#include <iostream>
#include <cassert>
#include <cstdint>
using Int=int; using Bool=bool; using UnsignedInt=uint32_t;
#define USE_PERF_TIMER(x)
#define DEBUG_ASSERTCRASH(x,msg) assert(x)
#define DEBUG_CRASH(msg) assert(false)
struct Module { uint32_t id,priority; int index=-1;
 uint32_t friend_getPriority()const{return priority;}
 void friend_setIndexInLogic(int i){index=i;}
};
using UpdateModulePtr=Module*;
struct GameLogic {std::vector<Module*> m_sleepyUpdates;
 void eraseSleepyUpdate(Int); Int rebalanceParentSleepyUpdate(Int);
 Int rebalanceChildSleepyUpdate(Int); void rebalanceSleepyUpdate(Int);
 void remakeSleepyUpdate(); void pushSleepyUpdate(Module*); void popSleepyUpdate();
 void validateSleepyUpdate(){}
};
'''
    cpp_driver += extract(cpp, 'inline Bool isLowerPriority(') + '\n' + '\n'.join(bodies)
    cpp_driver += r'''
int main(){GameLogic g; std::vector<Module> m(65);for(unsigned i=0;i<m.size();++i)m[i].id=i;
 char op; unsigned id,p;
 while(std::cin>>op>>id>>p){auto &x=m.at(id);
 if(op=='P'||op=='A'){if(x.index>=0)g.eraseSleepyUpdate(x.index);x.priority=p;
 if(op=='P')g.pushSleepyUpdate(&x);else{x.index=g.m_sleepyUpdates.size();g.m_sleepyUpdates.push_back(&x);}}
 if(op=='E'&&x.index>=0)g.eraseSleepyUpdate(x.index);
 if(op=='R'&&x.index>=0){x.priority=p;g.rebalanceSleepyUpdate(x.index);}
 if(op=='C'&&x.index>=0){x.priority=p;g.rebalanceSleepyUpdate(0);}
 if(op=='O'&&!g.m_sleepyUpdates.empty())g.popSleepyUpdate();
 if(op=='M')g.remakeSleepyUpdate();
 for(auto a:g.m_sleepyUpdates)std::cout<<a->id<<':'<<a->priority<<':'<<a->index<<',';
 std::cout<<'|';for(auto &a:m)std::cout<<a.id<<':'<<a.index<<',';std::cout<<'\n';}
}
'''
    rust_driver = '#[path = ' + json.dumps(str(rust_path)) + '] mod indexed_heap;\n'
    rust_driver += r'''
use indexed_heap::{IndexedHeap,IndexedHeapEntry}; use std::io::{self,Read};
fn main(){let mut h=IndexedHeap::new();let mut input=String::new();io::stdin().read_to_string(&mut input).unwrap();
for line in input.lines(){let a:Vec<_>=line.split_whitespace().collect();let id:u32=a[1].parse().unwrap();let p:u32=a[2].parse().unwrap();
match a[0]{"P"=>h.push(IndexedHeapEntry{key:id,priority:p,value:()}),
"A"=>{h.append_unbalanced(IndexedHeapEntry{key:id,priority:p,value:()});},
"E"=>{h.erase(id);},"R"=>{h.reschedule(id,p);},"C"=>{h.complete_root_update(id,p);},
"O"=>{h.pop();},"M"=>h.remake(),_=>panic!("unknown operation")}
for e in h.iter(){print!("{}:{}:{},",e.key,e.priority,h.index_of(e.key).unwrap());}
print!("|");for k in 0..65 {print!("{}:{},",k,h.index_of(k).map(|i|i as i32).unwrap_or(-1));}println!();}}
'''
    # Explicit probes plus reproducible mutation corpus. Priorities are packed
    # by actual unsigned wrap-then-clamp wake semantics, including all phases.
    def packed(frame, sleep, phase):
        return (min((frame + sleep) & 0xffffffff, 0x3fffffff) << 2) | phase
    ops = [('P', i, 10) for i in range(1, 9)]
    ops += [('O', 0, 0), ('E', 4, 0), ('R', 8, 20), ('P', 4, 0),
            ('P', 9, 0), ('C', 4, 24), ('E', 4, 0)]
    ops += [('A', i, (19-i)*4+(i%4)) for i in range(10, 19)] + [('M',0,0)]
    for i in range(19, 30):
        ops.append(('P', i, packed(0xfffffffe, i-18, i%4)))
    state = 0x13579bdf
    for i in range(600):
        state = (state*1664525+1013904223) & 0xffffffff
        key = state%40+1
        operation = ['P','E','R','O'][state>>16 & 3]
        ops.append((operation,key,((state>>24)%8)*4+i%4))
    data = ''.join(f'{op} {id} {priority}\n' for op,id,priority in ops)
    with tempfile.TemporaryDirectory(prefix='cpp-sleepy-heap-') as tmp:
        tmp = Path(tmp)
        (tmp/'cpp.cpp').write_text(cpp_driver)
        (tmp/'rust.rs').write_text(rust_driver)
        subprocess.run([cxx,'-std=c++17','-O0',str(tmp/'cpp.cpp'),'-o',str(tmp/'cpp')],check=True)
        subprocess.run([rustc,'--edition=2024','-C','debug-assertions=no',str(tmp/'rust.rs'),'-o',str(tmp/'rust')],check=True)
        outputs = [subprocess.run([str(tmp/name)],input=data,text=True,capture_output=True,check=True).stdout.splitlines() for name in ('cpp','rust')]
        if any(len(output)!=len(ops) for output in outputs):
            raise RuntimeError('Incomplete operation evidence')
        for i,(left,right) in enumerate(zip(*outputs)):
            if left!=right:
                raise RuntimeError(f'First difference at operation {i}: {ops[i]}\nC++ {left}\nRust {right}\nPrefix: {ops[:i+1]}')
    print(json.dumps({'scope':'indexed sleepy heap component only','operations':len(ops),
                      'cpp_sha256':hashlib.sha256(cpp.encode()).hexdigest(),
                      'rust_sha256':hashlib.sha256(rust.encode()).hexdigest(),
                      'cpp_branch':'original pristine #else child branch',
                      'compilers':{'cpp':cxx,'rust':rustc},'result':'match'},indent=2))

if __name__ == '__main__':
    main()
