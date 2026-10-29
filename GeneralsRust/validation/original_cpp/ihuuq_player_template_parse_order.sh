#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)
player_template_cpp="$repo_root/GeneralsMD/Code/GameEngine/Source/Common/RTS/PlayerTemplate.cpp"
ini_cpp="$repo_root/GeneralsMD/Code/GameEngine/Source/Common/INI/INI.cpp"
expected_player_hash=32d20a3dda6b16e8b1f4825d9bceb0aec3ecc2310d27c85d53e39f97b57a8de4
expected_ini_hash=a8aa77e908d2482aa774408be94cc9b7a1fa13fa4b48d0ea64311e85a9e3a5e7
actual_player_hash=$(shasum -a 256 "$player_template_cpp" | cut -d ' ' -f 1)
actual_ini_hash=$(shasum -a 256 "$ini_cpp" | cut -d ' ' -f 1)
test "$actual_player_hash" = "$expected_player_hash"
test "$actual_ini_hash" = "$expected_ini_hash"

scratch_dir=$(mktemp -d)
trap 'rm -rf "$scratch_dir"' EXIT HUP INT TERM
source_file="$scratch_dir/witness.cpp"
executable="$scratch_dir/witness"

cat > "$source_file" <<'CPP'
// Verbatim source excerpts are appended below. This is not the full Generals
// C++ build: only INI token state and NameKey allocation are minimal shims.
#include <cassert>
#include <cstdio>
#include <cstring>
#include <iostream>
#include <string>
#include <unordered_map>
#include <vector>

using Real = float;
using NameKeyType = unsigned int;
enum { INI_INVALID_DATA = 17 };
using IniError = decltype(INI_INVALID_DATA);

class NameKeyGenerator {
public:
    static void reset() { keys.clear(); next = 1; }
    static NameKeyType nameToKey(const char* name) {
        auto found = keys.find(name);
        if (found != keys.end()) return found->second;
        const auto key = next++;
        keys.emplace(name, key);
        return key;
    }
    static NameKeyType nextKey() { return next; }
private:
    static std::unordered_map<std::string, NameKeyType> keys;
    static NameKeyType next;
};
std::unordered_map<std::string, NameKeyType> NameKeyGenerator::keys;
NameKeyType NameKeyGenerator::next = 1;
#define NAMEKEY(token) NameKeyGenerator::nameToKey(token)

class INI {
public:
    static const char* getSeps() { return " \t\r\n"; }
    const char* getNextToken(const char* seps = nullptr);
    static Real scanPercentToReal(const char* token);
};

class PlayerTemplate {
public:
    std::unordered_map<NameKeyType, Real> m_productionCostChanges;
    std::unordered_map<NameKeyType, Real> m_productionTimeChanges;
    static void parseProductionCostChange(INI*, void*, void*, const void*);
    static void parseProductionTimeChange(INI*, void*, void*, const void*);
};
CPP

# Keep the original parser and INI token/scanner function bodies verbatim.
sed -n '110,118p' "$player_template_cpp" >> "$source_file"
sed -n '121,129p' "$player_template_cpp" >> "$source_file"
sed -n '1535,1542p' "$ini_cpp" >> "$source_file"
sed -n '1586,1592p' "$ini_cpp" >> "$source_file"

cat >> "$source_file" <<'CPP'
using Parser = void (*)(INI*, void*, void*, const void*);

void run_case(const char* label, Parser parser, const char* line,
              bool should_throw, NameKeyType expected_next, bool expect_value) {
    NameKeyGenerator::reset();
    std::vector<char> input(line, line + std::strlen(line) + 1);
    char* field = std::strtok(input.data(), " \t\r\n");
    assert(field != nullptr); // Field dispatch consumed the field-name token.
    INI ini;
    PlayerTemplate template_data;
    bool threw = false;
    try {
        parser(&ini, &template_data, nullptr, nullptr);
    } catch (IniError error) {
        assert(error == INI_INVALID_DATA);
        threw = true;
    }
    assert(threw == should_throw);
    assert(NameKeyGenerator::nextKey() == expected_next);
    if (expect_value) {
        const bool stored_cost = template_data.m_productionCostChanges.size() == 1
            && template_data.m_productionTimeChanges.empty();
        const bool stored_time = template_data.m_productionTimeChanges.size() == 1
            && template_data.m_productionCostChanges.empty();
        assert(stored_cost || stored_time);
        const Real stored = stored_cost ? template_data.m_productionCostChanges.begin()->second
                                         : template_data.m_productionTimeChanges.begin()->second;
        assert(stored == 0.75f);
    }
    std::cout << label << ": threw=" << threw
              << " next-name-key=" << NameKeyGenerator::nextKey() << '\n';
}

int main() {
    run_case("cost/missing-first-token", &PlayerTemplate::parseProductionCostChange,
             "ProductionCostChange", true, 1, false);
    run_case("cost/missing-second-token", &PlayerTemplate::parseProductionCostChange,
             "ProductionCostChange BuildName", true, 2, false);
    run_case("cost/invalid-percent", &PlayerTemplate::parseProductionCostChange,
             "ProductionCostChange BuildName nope", true, 2, false);
    run_case("cost/valid-percent", &PlayerTemplate::parseProductionCostChange,
             "ProductionCostChange BuildName 75%", false, 2, true);
    run_case("time/missing-first-token", &PlayerTemplate::parseProductionTimeChange,
             "ProductionTimeChange", true, 1, false);
    run_case("time/missing-second-token", &PlayerTemplate::parseProductionTimeChange,
             "ProductionTimeChange BuildName", true, 2, false);
    run_case("time/invalid-percent", &PlayerTemplate::parseProductionTimeChange,
             "ProductionTimeChange BuildName nope", true, 2, false);
    run_case("time/valid-percent", &PlayerTemplate::parseProductionTimeChange,
             "ProductionTimeChange BuildName 75%", false, 2, true);
}
CPP

${CXX:-clang++} -std=c++17 -Wall -Wextra -Werror -Wno-unused-parameter \
    "$source_file" -o "$executable"
"$executable"
