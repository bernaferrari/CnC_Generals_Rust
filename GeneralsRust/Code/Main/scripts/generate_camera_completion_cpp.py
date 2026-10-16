#!/usr/bin/env python3
"""Compile the original completion predicate and LookAt cancellation tail.

This component oracle omits terrain geometry, rendering and other camera logic.
Requires a C++ compiler; expected records come only from the extracted C++.
Filter ordinals are fixture labels, not claims about the original wire enums.
"""
from pathlib import Path
import argparse, hashlib, re, subprocess, tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--repo-root', type=Path, default=Path(__file__).resolve().parents[4])
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
source = args.repo_root / 'GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DView.cpp'
text = source.read_text()
predicate = re.search(r'Bool W3DView::isCameraMovementFinished\(void\)\s*\{(.*?)\n\}', text, re.S).group(1)
look = re.search(r'void W3DView::lookAt\( const Coord3D \*o \)\s*\{(.*?)\n\}', text, re.S).group(1)
tail = re.search(r'm_doingRotateCamera = false;.*?m_doingScriptedCameraLock = false;', look, re.S).group(0)
modes = ['FM_NULL', 'FM_BW', 'FM_RED', 'FM_GREEN', 'FM_CROSS',
 'FM_VIEW_MB_IN_AND_OUT_ALPHA', 'FM_VIEW_MB_IN_AND_OUT_SATURATE',
 'FM_VIEW_MB_IN_ALPHA', 'FM_VIEW_MB_OUT_ALPHA', 'FM_VIEW_MB_IN_SATURATE',
 'FM_VIEW_MB_OUT_SATURATE', 'FM_VIEW_MB_END_PAN', 'FM_VIEW_MB_PAN',
 'FM_VIEW_MB_PAN1', 'FM_VIEW_MB_PAN2', 'FM_VIEW_MB_PAN3']
program = r"""
#include <cstdio>
using Bool = bool;
enum Mode { @MODES@ };
enum { FT_NULL, FT_BW, FT_CROSS, FT_VIEW_MOTION_BLUR_FILTER };
struct View {
    int m_viewFilter, m_viewFilterMode;
    bool m_doingMoveCameraOnWaypointPath = false;
    bool m_doingRotateCamera, m_doingPitchCamera, m_doingZoomCamera;
    bool m_CameraArrivedAtWaypointOnPathFlag = false;
    bool m_doingScriptedCameraLock = false;
    bool finished() { @PREDICATE@ }
    void look() { @TAIL@ }
};
int main() {
    for (int mask = 0; mask < 8; ++mask) {
        for (int filter = 0; filter < 4; ++filter) {
            for (int mode = 0; mode < 16; ++mode) {
                View view;
                view.m_viewFilter = filter;
                view.m_viewFilterMode = mode;
                view.m_doingRotateCamera = mask & 1;
                view.m_doingPitchCamera = mask & 2;
                view.m_doingZoomCamera = mask & 4;
                bool before = view.finished();
                view.look();
                std::printf("%d,%d,%d,%d,%d,%d,%d,%d\n", mask, filter, mode,
                    before, view.finished(), view.m_doingRotateCamera,
                    view.m_doingPitchCamera, view.m_doingZoomCamera);
            }
        }
    }
}
""".replace('@MODES@', ','.join(modes)).replace('@PREDICATE@', predicate).replace('@TAIL@', tail)
with tempfile.TemporaryDirectory(prefix='camera-completion-cpp-') as directory:
    cpp, exe = Path(directory)/'probe.cpp', Path(directory)/'probe'
    cpp.write_text(program)
    subprocess.run(['clang++','-std=c++17',str(cpp),'-o',str(exe)],check=True)
    records = subprocess.check_output([str(exe)],text=True)
args.output.parent.mkdir(parents=True,exist_ok=True)
args.output.write_text('# C++ W3DView completion + LookAt flag tail; no geometry/full-engine claim.\n# source_sha256=' + hashlib.sha256(source.read_bytes()).hexdigest() + '\n# mask,filter,mode,before_finished,after_finished,rotate,pitch,zoom\n' + records)
print(f'{len(records.splitlines())} original component records -> {args.output}')
