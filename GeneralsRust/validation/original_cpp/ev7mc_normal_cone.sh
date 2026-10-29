#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)
cpp_root="$repo_root/GeneralsMD/Code/Libraries/Source/WWVegas/WWMath"
normalcone="$cpp_root/normalcone.h"
wwmath="$cpp_root/wwmath.h"

check_source_hash() {
    file=$1
    expected=$2
    actual=$(shasum -a 256 "$file" | awk '{print $1}')
    if [ "$actual" != "$expected" ]; then
        echo "source hash mismatch for $file: $actual" >&2
        exit 1
    fi
}

check_source_hash "$normalcone" 81b0bf43d6871f7ad936924797889054f1fc9b99ce7419971313c58b67652d60
check_source_hash "$wwmath" 1506b189fcead836247527a2a506522731a66fcf76f20c65b42d19cb8443efba

tmpdir=$(mktemp -d "${TMPDIR:-/tmp}/ev7mc-normal-cone.XXXXXX")
trap 'python3 -c '\''import pathlib, shutil, sys; shutil.rmtree(pathlib.Path(sys.argv[1]), ignore_errors=True)'\'' "$tmpdir"' EXIT HUP INT TERM

cat > "$tmpdir/witness.cpp" <<'EOF'
#include <stddef.h>
#include <cmath>

// The source header's build environment provides these through its platform
// prelude. The standalone witness aliases the source's Matrix3 spelling to
// the present Matrix3x3 type and suppresses debug-only assertions.
#define Matrix3 Matrix3x3
#define WWDEBUG_H
#include "normalcone.h"
#include <cstdio>

static bool unchanged(const NormalCone &cone) {
    return cone.X == 0.0f && cone.Y == 0.0f && cone.Z == 1.0f && cone.Angle == 1.0f;
}

int main() {
    NormalCone boundary(Vector3(0, 0, 1), -0.99995f);
    NormalCone outside(Vector3(0, 0, 1), -0.99989f);
    if (!boundary.Complete_Sphere() || outside.Complete_Sphere()) return 1;

    NormalCone cone(Vector3(0, 0, 1), 0.5f);
    Vector3 first, second;
    const float length = cone.Get_Coplanar_Normals(Vector3(1, 0, 0), first, second);
    const float expected = std::cos((1.0f - cone.Angle) * WWMATH_PI * 0.5f);
    if (std::fabs(length - 1.0f) > 0.0001f) return 2;
    if (std::fabs(Vector3::Dot_Product(first, cone) - expected) > 0.0001f) return 3;
    if (std::fabs(Vector3::Dot_Product(second, cone) - expected) > 0.0001f) return 4;

    NormalCone opposite(Vector3(0, 0, 1));
    opposite.Merge(Vector3(0, 0, -1));
    if (!unchanged(opposite)) return 5;

    NormalCone near_opposite(Vector3(0, 0, 1));
    near_opposite.Merge(Vector3(0.005f, 0, -0.9999875f));
    if (!unchanged(near_opposite)) return 6;

    std::puts("original normalcone.h boundary/coplanar/opposite cases PASS");
    return 0;
}
EOF

"${CXX:-clang++}" -std=c++17 -D__cdecl= '-DWWASSERT(x)=((void)0)' \
    -I "$cpp_root" \
    -I "$repo_root/GeneralsMD/Code/Libraries/Source/WWVegas/WWLib" \
    -I "$repo_root/GeneralsMD/Code/Libraries/Source/WWVegas/WWDebug" \
    "$tmpdir/witness.cpp" -o "$tmpdir/witness"
"$tmpdir/witness"
