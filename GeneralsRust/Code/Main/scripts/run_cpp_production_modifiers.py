#!/usr/bin/env python3
"""Source-pinned original parser/getter component, not a whole INI/game run."""
from pathlib import Path
import argparse
import hashlib
import json
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--repo', type=Path, default=Path(__file__).resolve().parents[4],
                    help='repository root (default: root containing this Main/scripts runner)')
parser.add_argument('--output', type=Path, required=True,
                    help='dedicated output directory for generated component, executable, CSV and receipt')
parser.add_argument('--cxx', default='c++', help='C++17 compiler executable')
args = parser.parse_args()
repo = args.repo.resolve()
output = args.output.resolve()
output.mkdir(parents=True, exist_ok=True)
base = repo / 'GeneralsMD/Code/GameEngine'
paths = {
    'template': base / 'Source/Common/RTS/PlayerTemplate.cpp',
    'player': base / 'Source/Common/RTS/Player.cpp',
    'template_header': base / 'Include/Common/PlayerTemplate.h',
    'player_header': base / 'Include/Common/Player.h',
    'ini': base / 'Source/Common/INI/INI.cpp',
    'name_key': base / 'Source/Common/NameKeyGenerator.cpp',
    'name_key_header': base / 'Include/Common/NameKeyGenerator.h',
    'common_header': base / 'Include/Common/GameCommon.h',
    'common': base / 'Source/Common/System/GameCommon.cpp',
    'typedefs': base / 'Include/Common/STLTypedefs.h',
}
EXPECTED_SOURCE_SHA256 = {'GeneralsMD/Code/GameEngine/Include/Common/GameCommon.h': '5765730dca375098a638184ce473cfbe43fed12623a1e8234ccd219883a5e54a',
 'GeneralsMD/Code/GameEngine/Include/Common/NameKeyGenerator.h': 'f84ca14cd15111249d3ef40b9fe447b23d7a470c6b828b83c7a65c6e9e557a1f',
 'GeneralsMD/Code/GameEngine/Include/Common/Player.h': '4c20d95fd4cfcf963b90e4a347d55cb6a3e7c6e72853db2c2de2e60f45469396',
 'GeneralsMD/Code/GameEngine/Include/Common/PlayerTemplate.h': '5094b42372daf3ec22755fed6ba06ab0aec1a03530e8960149ff95bccf4ab5b6',
 'GeneralsMD/Code/GameEngine/Include/Common/STLTypedefs.h': '0c8e2c84f628a239ad78afe08ad9a73df175687e07fdd2bd05ce771d523fcda4',
 'GeneralsMD/Code/GameEngine/Source/Common/INI/INI.cpp': 'a8aa77e908d2482aa774408be94cc9b7a1fa13fa4b48d0ea64311e85a9e3a5e7',
 'GeneralsMD/Code/GameEngine/Source/Common/NameKeyGenerator.cpp': '0d30c6f548c662746d162a563591fc4bfe6589fe412a5ecd5052d6dd09f83a8b',
 'GeneralsMD/Code/GameEngine/Source/Common/RTS/Player.cpp': 'b9b35cc1ac3eb053597c45b2992f7c1b15b6daa19f9344d1fecd67c182059519',
 'GeneralsMD/Code/GameEngine/Source/Common/RTS/PlayerTemplate.cpp': '32d20a3dda6b16e8b1f4825d9bceb0aec3ecc2310d27c85d53e39f97b57a8de4',
 'GeneralsMD/Code/GameEngine/Source/Common/System/GameCommon.cpp': 'd18d9f8ff9cee817d7bf88b370dc2e4a701f98979c85cd3bd0c712bd18818157'}
for path in paths.values():
    relative = str(path.relative_to(repo))
    actual = hashlib.sha256(path.read_bytes()).hexdigest()
    if actual != EXPECTED_SOURCE_SHA256[relative]:
        raise SystemExit(f'source pin changed for {relative}: {actual}; review the original component before updating pins')
sources = {key: path.read_text() for key, path in paths.items()}
ranges = []


def slice_at(key, start, stop):
    source = sources[key]
    text = source[start:stop]
    ranges.append({
        'path': str(paths[key].relative_to(repo)),
        'lines': [source[:start].count('\n') + 1, source[:stop - 1].count('\n') + 1],
        'sha256': hashlib.sha256(text.encode()).hexdigest(),
    })
    return text


def definition(key, signature, terminated=False):
    source = sources[key]
    start = source.index(signature)
    brace = source.index('{', start)
    nesting = 1
    stop = brace + 1
    while nesting:
        nesting += (source[stop] == '{') - (source[stop] == '}')
        stop += 1
    if terminated:
        assert source[stop] == ';'
        stop += 1
    return slice_at(key, start, stop)


def statement(key, text):
    start = sources[key].index(text)
    return slice_at(key, start, start + len(text))


parsers = [definition('template', f'void PlayerTemplate::{name}(') for name in [
    'parseProductionCostChange', 'parseProductionTimeChange', 'parseProductionVeterancyLevel',
]]
getters = [definition('player', f'{result} Player::{name}(') for result, name in [
    ('Real', 'getProductionCostChangePercent'), ('Real', 'getProductionTimeChangePercent'),
    ('VeterancyLevel', 'getProductionVeterancyLevel'),
]]
scanners = [definition('ini', signature) for signature in [
    'Real INI::scanPercentToReal(', 'Int INI::scanIndexList(',
]]
name_methods = [definition('name_key', signature) for signature in [
    'NameKeyGenerator::NameKeyGenerator()', 'NameKeyGenerator::~NameKeyGenerator()',
    'void NameKeyGenerator::init()', 'void NameKeyGenerator::reset()',
    'void NameKeyGenerator::freeSockets()', 'inline UnsignedInt calcHashForString(',
    'NameKeyType NameKeyGenerator::nameToKey(',
]]
name_enum = definition('name_key_header', 'enum NameKeyType', True)
veterancy_enum = definition('common_header', 'enum VeterancyLevel', True)
veterancy_names = definition('common', 'const char *TheVeterancyNames[] =', True)
production_maps = [statement('typedefs', text) for text in [
    'typedef std::map< NameKeyType, Real, std::less<NameKeyType> > ProductionChangeMap;',
    'typedef std::map< NameKeyType, VeterancyLevel, std::less<NameKeyType> > ProductionVeterancyMap;',
]]
inline_getters = [definition('template_header', signature) for signature in [
    'inline const ProductionChangeMap& getProductionCostChanges()',
    'inline const ProductionChangeMap& getProductionTimeChanges()',
    'inline const ProductionVeterancyMap& getProductionVeterancyLevels()',
]]
map_copy = [statement('player', text) for text in [
    'm_productionCostChanges = pt->getProductionCostChanges();',
    'm_productionTimeChanges = pt->getProductionTimeChanges();',
    'm_productionVeterancyLevels = pt->getProductionVeterancyLevels();',
]]

adapters = r'''
#include <cstdint>
#include <cstdio>
#include <cstring>
#ifndef _WIN32
#include <strings.h>
#endif
#include <initializer_list>
#include <iostream>
#include <map>
#include <stdexcept>
#include <string>
#include <vector>
using Real = float;
using Int = int32_t;
using UnsignedInt = uint32_t;
using Byte = unsigned char;
using ConstCharPtrArray = const char* const*;
constexpr Int INI_INVALID_DATA = 1;
constexpr Int INI_INVALID_NAME_LIST = 2;
#define DEBUG_ASSERTCRASH(condition, message) do { if (!(condition)) throw std::runtime_error("original assertion"); } while (false)
#define DEBUG_CRASH(message) do {} while (false)
#define newInstance(type) new type
#ifdef _WIN32
#define stricmp _stricmp
#else
#define stricmp strcasecmp
#endif
class AsciiString {
    std::string text;
public:
    AsciiString() = default;
    AsciiString(const char* value) : text(value) {}
    const char* str() const { return text.c_str(); }
    AsciiString& operator=(const char* value) { text = value; return *this; }
};
'''
name_adapter = r'''
struct Bucket {
    Bucket* m_nextInSocket = nullptr;
    NameKeyType m_key = NAMEKEY_INVALID;
    AsciiString m_nameString;
    void deleteInstance() { delete this; }
};
class NameKeyGenerator {
    static constexpr Int SOCKET_COUNT = 45007;
    Bucket* m_sockets[SOCKET_COUNT];
    UnsignedInt m_nextID;
    void freeSockets();
public:
    NameKeyGenerator();
    ~NameKeyGenerator();
    void init();
    void reset();
    NameKeyType nameToKey(const char* name);
};
NameKeyGenerator* TheNameKeyGenerator;
NameKeyType NAMEKEY(const char* name) { return TheNameKeyGenerator->nameToKey(name); }
NameKeyType NAMEKEY(const AsciiString& name) { return NAMEKEY(name.str()); }
class INI {
    std::vector<std::string> tokens;
    size_t next = 0;
public:
    INI(std::initializer_list<std::string> values) : tokens(values) {}
    const char* getNextToken() {
        if (next == tokens.size()) throw INI_INVALID_DATA;
        return tokens[next++].c_str();
    }
    static Real scanPercentToReal(const char* token);
    static Int scanIndexList(const char* token, ConstCharPtrArray names);
};
'''
classes = r'''
class PlayerTemplate {
public:
    ProductionChangeMap m_productionCostChanges, m_productionTimeChanges;
    ProductionVeterancyMap m_productionVeterancyLevels;
    static void parseProductionCostChange(INI*, void*, void*, const void*);
    static void parseProductionTimeChange(INI*, void*, void*, const void*);
    static void parseProductionVeterancyLevel(INI*, void*, void*, const void*);
''' + '\n'.join(inline_getters) + r'''
};
class Player {
public:
    ProductionChangeMap m_productionCostChanges, m_productionTimeChanges;
    ProductionVeterancyMap m_productionVeterancyLevels;
    Real getProductionCostChangePercent(AsciiString) const;
    Real getProductionTimeChangePercent(AsciiString) const;
    VeterancyLevel getProductionVeterancyLevel(AsciiString) const;
    void copyInitMaps(PlayerTemplate* pt) {
''' + '\n'.join(map_copy) + r'''
    }
};
'''
main = r'''
UnsignedInt bits(Real value) { UnsignedInt result; std::memcpy(&result, &value, sizeof(value)); return result; }
void record(const char* phase, const Player& player, const char* name) {
    std::cout << phase << ',' << name << ','
              << bits(player.getProductionCostChangePercent(name)) << ','
              << bits(player.getProductionTimeChangePercent(name)) << ','
              << player.getProductionVeterancyLevel(name) << '\n';
}
void parse(PlayerTemplate& pt, const char* field, const char* name, const char* value) {
    INI ini{name, value};
    if (std::strcmp(field, "cost") == 0) PlayerTemplate::parseProductionCostChange(&ini, &pt, nullptr, nullptr);
    else if (std::strcmp(field, "time") == 0) PlayerTemplate::parseProductionTimeChange(&ini, &pt, nullptr, nullptr);
    else PlayerTemplate::parseProductionVeterancyLevel(&ini, &pt, nullptr, nullptr);
}
void scenario(const char* phase, Int preallocated) {
    NameKeyGenerator keys; keys.init(); TheNameKeyGenerator = &keys;
    for (Int i = 0; i < preallocated; ++i) NAMEKEY(("unrelated_" + std::to_string(i)).c_str());
    PlayerTemplate pt;
    parse(pt, "cost", "USAJet", "125%");
    parse(pt, "time", "USAJet", "-25%");
    parse(pt, "veterancy", "USAJet", "VETERAN");
    parse(pt, "cost", "USAInfantry", "37.5%");
    parse(pt, "time", "USAInfantry", "50%");
    parse(pt, "veterancy", "USAInfantry", "ELITE");
    parse(pt, "cost", "usaJet", "-10%");
    parse(pt, "time", "usaJet", "25%");
    parse(pt, "veterancy", "usaJet", "regular");
    Player before; before.copyInitMaps(&pt);
    for (auto name : {"USAJet", "USAInfantry", "usaJet", "USAJET", "Unknown"}) record(phase, before, name);
    parse(pt, "cost", "USAJet", "37.5%");
    parse(pt, "time", "USAJet", "75%");
    parse(pt, "veterancy", "USAJet", "hErOiC");
    Player after; after.copyInitMaps(&pt);
    const std::string finalPhase = std::string(phase) + "_override";
    for (auto name : {"USAJet", "USAInfantry", "usaJet", "USAJET", "Unknown"}) record(finalPhase.c_str(), after, name);
    // Player owns copied template maps; later template overrides do not alter it.
    record((std::string(phase) + "_retained").c_str(), before, "USAJet");
}
int main() { scenario("initial", 0); scenario("preallocated", 73); }
'''
driver_text = '\n'.join([adapters, name_enum, veterancy_enum, veterancy_names,
                         '\n'.join(production_maps), name_adapter,
                         '\n'.join(name_methods), '\n'.join(scanners), classes,
                         '\n'.join(parsers), '\n'.join(getters), main])
driver = output / 'production_modifiers.cpp'
driver.write_text(driver_text)
executable = output / 'cpp-production-modifiers'
command = [args.cxx, '-std=c++17', '-O2', '-Wall', '-Wextra', '-Wpedantic',
           '-fsanitize=undefined', '-fno-sanitize-recover=all', str(driver), '-o', str(executable)]
compiled = subprocess.run(command, text=True, capture_output=True)
(output / 'compile.stdout').write_text(compiled.stdout)
(output / 'compile.stderr').write_text(compiled.stderr)
compiled.check_returncode()
result = subprocess.run([str(executable)], capture_output=True)
(output / 'modifiers.csv').write_bytes(result.stdout)
(output / 'execution.stderr').write_bytes(result.stderr)
result.check_returncode()
records = result.stdout.decode().splitlines()
assert len(records) == 22
assert all(len(line.split(',')) == 5 for line in records)
receipt = {
    'bead': 'hq-iriod',
    'command': command,
    'execution_command': [str(executable)],
    'compiler': subprocess.check_output([args.cxx, '--version'], text=True).splitlines()[0],
    'runner_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    'sources': {str(path.relative_to(repo)): hashlib.sha256(path.read_bytes()).hexdigest()
                for path in paths.values()},
    'extracted_ranges': ranges,
    'driver_sha256': hashlib.sha256(driver.read_bytes()).hexdigest(),
    'executable_sha256': hashlib.sha256(executable.read_bytes()).hexdigest(),
    'output_sha256': hashlib.sha256(result.stdout).hexdigest(),
    'records': len(records),
    'columns': ['phase', 'name', 'cost_bits', 'time_bits', 'veterancy'],
    'scope': 'Executed exact extracted original PlayerTemplate parser functions, original INI percent/index scanners, original NameKeyGenerator case-sensitive lookup/hash/lifecycle, Player::init map-copy statements, and Player production getters. Tokens, strings and memory allocation use bounded compile adapters; no complete original INI dispatch, worker handoff, game production, cost/time rounding, save continuation or match behavior is proved.',
    'adapters': ['32-bit primitive type aliases', 'owned AsciiString storage', 'explicit two-token INI cursor', 'heap Bucket allocation/deletion instead of memory pool', 'stricmp alias to platform case-insensitive C string comparison', 'non-triggered debug macro adapters', 'bounded declarations containing the original production maps'],
}
(output / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
print(result.stdout.decode(), end='')
