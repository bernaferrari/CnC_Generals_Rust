#!/usr/bin/env python3
"""Original source-pinned saved-upgrade lookup and timing component witness."""
from pathlib import Path
import argparse
import csv
import hashlib
import json
import re
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--repo-root', '--repo', dest='repo_root', type=Path,
                    default=Path(__file__).resolve().parents[4],
                    help='repository root (default: root containing this Main/scripts runner)')
parser.add_argument('--output', type=Path, required=True,
                    help='dedicated output directory for generated component, executable, CSV and receipt')
parser.add_argument('--cxx', default='c++', help='C++17 compiler executable')
args = parser.parse_args()
repo, output = args.repo_root.resolve(), args.output.resolve()
output.mkdir(parents=True, exist_ok=True)
PINS = {
 'GeneralsMD/Code/GameEngine/Source/Common/System/Upgrade.cpp': '26a9695577fd5d3a527662384e951d8f4743c5f851d34389d5ac63e18fc0d823',
 'GeneralsMD/Code/GameEngine/Include/Common/Upgrade.h': '1c2c436b5598f560c41968dde295eea579e18b9c0ef13c4394fa516b389b4635',
 'GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/ProductionUpdate.cpp': 'a35cefe80c7af3735d519114568a16511f799ed4ae1b59581214e317e6d3efe4',
 'GeneralsMD/Code/GameEngine/Include/GameLogic/Module/ProductionUpdate.h': '9f05cc6ffa33bf8b7957a1aee4b351868ac50ff9f3d7bc599c3a1f4e5ba93478',
 'GeneralsMD/Code/GameEngine/Include/Common/GameState.h': '7f01faa4b49e538469307c3e0b3e8871e4b661400155b3871f744c587d22a03d',
 'GeneralsMD/Code/Libraries/Include/Lib/BaseType.h': '590af4e59d0e48510d976eeafa248000a1ac9f05d8f905f66f24b1c1cf9eceef',
 'GeneralsMD/Code/GameEngine/Source/Common/NameKeyGenerator.cpp': '0d30c6f548c662746d162a563591fc4bfe6589fe412a5ecd5052d6dd09f83a8b',
 'GeneralsMD/Code/GameEngine/Include/Common/NameKeyGenerator.h': 'f84ca14cd15111249d3ef40b9fe447b23d7a470c6b828b83c7a65c6e9e557a1f',
 'GeneralsMD/Code/GameEngine/Include/Common/GameCommon.h': '5765730dca375098a638184ce473cfbe43fed12623a1e8234ccd219883a5e54a',
 'GeneralsMD/Code/GameEngine/Include/Common/Player.h': '4c20d95fd4cfcf963b90e4a347d55cb6a3e7c6e72853db2c2de2e60f45469396',
}
for name, expected in PINS.items():
    actual = hashlib.sha256((repo / name).read_bytes()).hexdigest()
    if actual != expected:
        raise SystemExit(f'original source pin changed: {name}: {actual}; review before updating')
sources = {name: (repo / name).read_text() for name in PINS}
ranges = []

def record(path, start, stop, description):
    src, text = sources[path], sources[path][start:stop]
    ranges.append({'path': path, 'description': description,
                   'lines': [src[:start].count('\n') + 1, src[:stop-1].count('\n') + 1],
                   'sha256': hashlib.sha256(text.encode()).hexdigest()})
    return text

def definition(path, signature, terminated=False):
    src = sources[path]
    start = src.index(signature)
    brace = src.index('{', start)
    nesting, stop = 1, brace + 1
    while nesting:
        nesting += (src[stop] == '{') - (src[stop] == '}')
        stop += 1
    if terminated:
        assert src[stop] == ';'
        stop += 1
    return record(path, start, stop, signature)

def fragment(path, begin, end):
    start = sources[path].index(begin)
    stop = sources[path].index(end, start)
    return record(path, start, stop, begin)

def statement(path, text):
    start = sources[path].index(text)
    return record(path, start, start + len(text), text)

base = 'GeneralsMD/Code/GameEngine/'
types_path = 'GeneralsMD/Code/Libraries/Include/Lib/BaseType.h'
upgrade_path = base + 'Source/Common/System/Upgrade.cpp'
upgrade_header = base + 'Include/Common/Upgrade.h'
production_path = base + 'Source/GameLogic/Object/Update/ProductionUpdate.cpp'
name_path = base + 'Source/Common/NameKeyGenerator.cpp'
name_header = base + 'Include/Common/NameKeyGenerator.h'
type_defs = []
for name in ['Real', 'Int', 'UnsignedInt', 'Bool']:
    match = re.search(r'^typedef[^;\n]*\b' + name + r';', sources[types_path], re.M)
    type_defs.append(record(types_path, match.start(), match.end(), f'typedef {name}'))
int_to_real = record(types_path, sources[types_path].index('#define INT_TO_REAL(x)'),
                     sources[types_path].index('\n', sources[types_path].index('#define INT_TO_REAL(x)')),
                     'INT_TO_REAL original macro')
name_enum = definition(name_header, 'enum NameKeyType', True)
name_forward = definition(name_header, 'NameKeyType nameToKey(const AsciiString& name)')
save_codes = definition(base + 'Include/Common/GameState.h', 'enum SaveCode', True)
production_types = definition(base + 'Include/GameLogic/Module/ProductionUpdate.h', 'enum ProductionType', True)
logic_constants = definition(base + 'Include/Common/GameCommon.h', 'enum\n{\n\tLOGICFRAMES_PER_SECOND', True)
name_methods = [definition(name_path, signature) for signature in [
    'NameKeyGenerator::NameKeyGenerator()', 'NameKeyGenerator::~NameKeyGenerator()',
    'void NameKeyGenerator::init()', 'void NameKeyGenerator::reset()',
    'void NameKeyGenerator::freeSockets()', 'inline UnsignedInt calcHashForString(',
    'NameKeyType NameKeyGenerator::nameToKey(',
]]
upgrade_time = definition(upgrade_path, 'Int UpgradeTemplate::calcTimeToBuild(')
upgrade_cost = definition(upgrade_path, 'Int UpgradeTemplate::calcCostToBuild(')
center_methods = [definition(upgrade_path, signature) for signature in [
    'const UpgradeTemplate *UpgradeCenter::findUpgradeByKey(',
    'const UpgradeTemplate *UpgradeCenter::findUpgrade(',
]]
upgrade_getters = [definition(upgrade_header, signature) for signature in [
    'NameKeyType getUpgradeNameKey(', 'const UpgradeTemplate *friend_getNext(',
]]
restore_block = fragment(production_path,
    'production->m_upgradeToResearch = TheUpgradeCenter->findUpgrade( name );',
    '\n\t\t\t}  // end else, upgrade production')
progress_body = fragment(production_path,
    'production->m_framesUnderConstruction++;',
    '\n\t{\n\n\t\t// handle unit production items')
withdraw_block = fragment(production_path,
    'Money *money = player->getMoney();\n\tmoney->withdraw( upgrade->calcCostToBuild( player ) );',
    '\n\n\t// allocate a new production entry')
deposit_block = fragment(production_path,
    'Money *money = player->getMoney();\n\tmoney->deposit( production->m_upgradeToResearch->calcCostToBuild( player ) );',
    '\n\n\t// remove this production from the queue')
money_getter = definition(base + 'Include/Common/Player.h', 'inline Money *getMoney()')

includes = r'''
#include <cstdint>
#include <cstring>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <limits>
#include <stdexcept>
#include <string>
using Byte = unsigned char;
#define DEBUG_ASSERTCRASH(condition, message) do { if (!(condition)) throw std::runtime_error("original assertion"); } while (false)
#define DEBUG_CRASH(message) do {} while (false)
#define newInstance(type) new type
class AsciiString {
    std::string text;
public:
    AsciiString() = default;
    AsciiString(const char* s) : text(s) {}
    const char* str() const { return text.c_str(); }
    AsciiString& operator=(const char* s) { text = s; return *this; }
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
    NameKeyType nameToKey(const char*);
''' + name_forward + r'''
};
NameKeyGenerator* TheNameKeyGenerator;
class Money {
public:
    Int withdrawCalls = 0, depositCalls = 0;
    UnsignedInt lastWithdraw = 0, lastDeposit = 0;
    // Argument-capture adapter only: original Money balance/audio/stats do not execute.
    UnsignedInt withdraw(UnsignedInt value) { ++withdrawCalls; lastWithdraw = value; return value; }
    void deposit(UnsignedInt value) { ++depositCalls; lastDeposit = value; }
};
class Player {
public:
    Money m_money;
''' + money_getter + r'''
};
class UpgradeTemplate {
public:
    Real m_buildTime = 0.0f;
    Int m_cost = 0;
    NameKeyType m_nameKey = NAMEKEY_INVALID;
    UpgradeTemplate* m_next = nullptr;
    Int calcTimeToBuild(Player*) const;
    Int calcCostToBuild(Player*) const;
''' + '\n'.join(upgrade_getters) + r'''
};
class UpgradeCenter {
public:
    UpgradeTemplate* m_upgradeList = nullptr;
    const UpgradeTemplate* findUpgradeByKey(NameKeyType) const;
    const UpgradeTemplate* findUpgrade(const AsciiString&) const;
};
UpgradeCenter* TheUpgradeCenter;
class ThingTemplate {
public:
    Int calcTimeToBuild(Player*) const { throw std::runtime_error("unit branch outside witness"); }
};
struct ProductionEntry {
    ProductionType m_type = PRODUCTION_UPGRADE;
    const UpgradeTemplate* m_upgradeToResearch = nullptr;
    const ThingTemplate* m_objectToProduce = nullptr;
    Int m_framesUnderConstruction = 0;
    Real m_percentComplete = 0.0f;
};
'''
restore_wrapper = 'void resolveSavedUpgrade(ProductionEntry* production, const AsciiString& name) {\n' + restore_block + '\n}\n'
# The original fragment includes its completion IF condition. Only the completion effect is replaced
# with an observation return; there is no upgrade installation/event/queue-removal claim.
progress_wrapper = 'Bool progressCompletionCondition(ProductionEntry* production, Player* player) {\n' + progress_body + '\n        return true;\n    return false;\n}\n'
withdraw_wrapper = 'void observeQueueWithdraw(Player* player, const UpgradeTemplate* upgrade) {\n' + withdraw_block + '\n}\n'
deposit_wrapper = 'void observeCancelDeposit(Player* player, ProductionEntry* production) {\n' + deposit_block + '\n}\n'
main = r'''
uint32_t bits(Real value) { uint32_t out; std::memcpy(&out, &value, sizeof(out)); return out; }
void restoreRow(std::ostream& out, const char* label, const char* name) {
    ProductionEntry entry;
    const char* outcome = "resolved";
    Int error = SC_OK;
    try { resolveSavedUpgrade(&entry, AsciiString(name)); }
    catch (SaveCode code) { error = code; outcome = code == SC_INVALID_DATA ? "invalid_data" : "other_error"; }
    out << label << ',' << name << ',' << outcome << ',' << error << ','
        << (entry.m_upgradeToResearch != nullptr) << '\n';
}
void timingRow(std::ostream& out, const char* label, Real seconds, Int elapsed) {
    Player player;
    UpgradeTemplate upgrade;
    upgrade.m_buildTime = seconds;
    upgrade.m_cost = 37;
    ProductionEntry entry;
    entry.m_upgradeToResearch = &upgrade;
    entry.m_framesUnderConstruction = elapsed;
    entry.m_percentComplete = 12.5f; // Supplied saved percentage, overwritten by original next-step calculation.
    const Int total = upgrade.calcTimeToBuild(&player);
    const Int rawCost = upgrade.calcCostToBuild(&player);
    Int firstCompletionTick = 0;
    uint32_t firstPercent = 0;
    for (Int tick = 1; tick <= 1000; ++tick) {
        const Bool completed = progressCompletionCondition(&entry, &player);
        if (tick == 1) firstPercent = bits(entry.m_percentComplete);
        if (completed) { firstCompletionTick = tick; break; }
    }
    if (firstCompletionTick == 0) throw std::runtime_error("case did not reach original completion condition");
    out << label << ',' << std::hex << std::setfill('0') << std::setw(8) << bits(seconds) << std::dec
        << ',' << elapsed << ',' << total << ',' << rawCost << ',' << firstCompletionTick << ','
        << entry.m_framesUnderConstruction << ',' << std::hex << std::setw(8) << firstPercent << ','
        << std::setw(8) << bits(entry.m_percentComplete) << std::dec << '\n';
}
void moneyRow(std::ostream& out, const char* label, Int cost) {
    Player player;
    UpgradeTemplate upgrade;
    upgrade.m_cost = cost;
    ProductionEntry entry;
    entry.m_upgradeToResearch = &upgrade;
    const Int rawCost = upgrade.calcCostToBuild(&player);
    observeQueueWithdraw(&player, &upgrade);
    observeCancelDeposit(&player, &entry);
    out << label << ',' << rawCost << ',' << player.m_money.withdrawCalls << ',' << player.m_money.lastWithdraw
        << ',' << player.m_money.depositCalls << ',' << player.m_money.lastDeposit << '\n';
}
int main(int argc, char** argv) {
    static_assert(sizeof(Real) == 4 && sizeof(Int) == 4 && sizeof(UnsignedInt) == 4);
    static_assert(std::numeric_limits<Real>::is_iec559);
    if (argc != 2) throw std::runtime_error("output directory required");
    NameKeyGenerator keys;
    keys.init();
    TheNameKeyGenerator = &keys;
    UpgradeTemplate known;
    known.m_nameKey = keys.nameToKey("Upgrade_Test");
    UpgradeCenter center;
    center.m_upgradeList = &known;
    TheUpgradeCenter = &center;
    std::ofstream restores(std::string(argv[1]) + "/restore.csv");
    std::ofstream timings(std::string(argv[1]) + "/timing.csv");
    std::ofstream money(std::string(argv[1]) + "/money_arguments.csv");
    if (!restores || !timings || !money) throw std::runtime_error("cannot create CSV");
    restores << "case,saved_name,outcome,error_code,resolved_pointer\n";
    restoreRow(restores, "exact_saved_name", "Upgrade_Test");
    restoreRow(restores, "absent_saved_name", "Upgrade_Missing");
    restoreRow(restores, "case_different_saved_name", "upgrade_test");
    restoreRow(restores, "empty_saved_name", "");
    timings << "case,authored_seconds_bits,restored_elapsed,original_total_frames,original_raw_cost,first_completion_tick,elapsed_at_completion,first_step_percent_bits,completion_percent_bits\n";
    timingRow(timings, "zero_elapsed0", 0.0f, 0);
    timingRow(timings, "zero_elapsed7", 0.0f, 7);
    timingRow(timings, "subframe_elapsed0", 0.005f, 0);
    timingRow(timings, "subframe_elapsed7", 0.005f, 7);
    timingRow(timings, "fractional125_elapsed0", 1.25f, 0);
    timingRow(timings, "fractional125_elapsed7", 1.25f, 7);
    timingRow(timings, "fractional125_elapsed37", 1.25f, 37);
    timingRow(timings, "fractional125_elapsed45", 1.25f, 45);
    money << "case,original_raw_cost,withdraw_calls,withdraw_argument,deposit_calls,deposit_argument\n";
    moneyRow(money, "authored_free_upgrade", 0);
    moneyRow(money, "authored_upgrade_cost37", 37);
    moneyRow(money, "authored_upgrade_cost125", 125);
    restores.close(); timings.close(); money.close();
    if (!restores || !timings || !money) throw std::runtime_error("CSV write failed");
    std::cout << "Executed original upgrade lookup, saved-name rejection, release timing/cost and progress-condition components.\n";
}
'''
cpp_text = '\n\n'.join([includes, '\n'.join(type_defs), int_to_real, name_enum, save_codes,
                         production_types, logic_constants, name_adapter,
                         '\n'.join(name_methods), upgrade_time, upgrade_cost,
                         '\n'.join(center_methods), restore_wrapper, progress_wrapper,
                         withdraw_wrapper, deposit_wrapper, main])
cpp = output / 'upgrade_restore.cpp'
cpp.write_text(cpp_text)
binary = output / 'cpp-upgrade-restore'
compile_command = [args.cxx, '-std=c++17', '-O2', '-Wall', '-Wextra', '-Wpedantic', '-fno-fast-math',
                   '-ffp-contract=off', '-fsanitize=undefined', '-fno-sanitize-recover=all', str(cpp), '-o', str(binary)]
compiled = subprocess.run(compile_command, text=True, capture_output=True)
(output / 'compile.stdout').write_text(compiled.stdout)
(output / 'compile.stderr').write_text(compiled.stderr)
run_command = [str(binary), str(output)]
executed = None
if compiled.returncode == 0:
    executed = subprocess.run(run_command, text=True, capture_output=True)
    (output / 'execution.stdout').write_text(executed.stdout)
    (output / 'execution.stderr').write_text(executed.stderr)
version = subprocess.run([args.cxx, '--version'], text=True, capture_output=True)
files = {}
for name in ['restore.csv', 'timing.csv', 'money_arguments.csv']:
    path = output / name
    if path.exists():
        rows = list(csv.DictReader(path.open()))
        files[name] = {'sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'records': len(rows),
                       'cases': [row['case'] for row in rows]}
receipt = {
    'issues': ['hq-p6s42', 'hq-7chz9'],
    'scope': 'Original C++ saved-upgrade lookup and timing/progress-condition component, not full queue or save-game execution',
    'source_sha256': PINS, 'extracted_source_ranges': ranges,
    'generated_source_sha256': hashlib.sha256(cpp.read_bytes()).hexdigest(),
    'runner_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    'compiler_version': version.stdout, 'compile_command': compile_command, 'compile_exit': compiled.returncode,
    'run_command': run_command, 'run_exit': executed.returncode if executed else None,
    'executable_sha256': hashlib.sha256(binary.read_bytes()).hexdigest() if binary.exists() else None,
    'csv': files,
    'executed_original': [
        'NameKeyGenerator constructor/destructor/init/reset/freeSockets/hash/nameToKey and AsciiString forwarding wrapper',
        'UpgradeCenter::findUpgrade/findUpgradeByKey using original exact NameKey matching and linked-list visitation',
        'UpgradeTemplate release calcTimeToBuild and calcCostToBuild, name-key/next wrappers',
        'ProductionUpdate::xfer saved-upgrade findUpgrade/null-rejection block only',
        'ProductionUpdate::update increment, total-production-time selection, percent calculation and completion IF condition only',
        'ProductionUpdate queue/cancel getMoney and withdraw/deposit calls only',
        'Original numeric typedefs, INT_TO_REAL macro, 30-frame constants, ProductionType/SaveCode/NameKeyType enums',
    ],
    'adapters_and_limits': [
        'Upgrade templates and linked list are supplied fixture definitions; INI parsing/default inheritance/mask allocation do not execute.',
        'AsciiString/std::string and ordinary Bucket allocation adapt string/memory infrastructure; original NameKey identity comparison executes.',
        'The saved name is supplied directly to the exact original xfer rejection block; binary Xfer, serialized queue layout, and whole save loading do not execute.',
        'ProductionEntry type/elapsed/percent/pointers and Player are supplied adapters. Upgrade-only progress arithmetic executes; unit branch throws if reached.',
        'Original completion IF condition executes, with completion body replaced by an observation return. Upgrade installation, events, queue removal and full update guards do not execute.',
        'Elapsed frames 0/7/37/45 are supplied restored state; saved percentage is supplied as 12.5 and overwritten by the original next-step calculation.',
        'Zero-frame progress performs the original floating division by zero on IEEE-754 float and observes positive infinity; no integer division occurs. This is not a claim that every possible original input is safe.',
        'Money adapter captures withdraw/deposit arguments only. Original balance changes, audio, statistics, affordability, admission/cancel guards are outside scope.',
        'No power/General/handicap change is supplied or executed. Their absence from original release upgrade timing/cost bodies is source evidence, not an executed power system claim.',
        'Release branch excludes debug instant construction. No full game, map load, Rust tests/build, RNG, network, rendering or retail compiler equivalence claim.',
    ],
}
(output / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
if compiled.returncode:
    raise SystemExit(f'compile failed; see {output / "compile.stderr"}')
if not executed or executed.returncode:
    raise SystemExit(f'execution failed; see {output / "execution.stderr"}')
print(json.dumps({'compile_exit': compiled.returncode, 'run_exit': executed.returncode,
                  'records': {name: data['records'] for name, data in files.items()},
                  'receipt': str(output / 'receipt.json')}))
