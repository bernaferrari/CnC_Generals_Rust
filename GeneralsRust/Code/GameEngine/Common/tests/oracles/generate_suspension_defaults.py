#!/usr/bin/env python3
"""Pinned five-field constructor/parser oracle, not the original engine.

Extracts active constructor assignments (excluding commented zero assignments),
Real typedef, and original scanReal/parseReal methods. Tokens are a narrow valid
finite decimal corpus; this does not claim complete lexer or rendering parity.
Run --output OUTSIDE the repository, then compare the frozen fixture.
"""
import argparse
import hashlib
from pathlib import Path
import re
import subprocess
from generate_ini_numeric import PINS, extract_method
from generate_locomotor_numeric import LOCOMOTOR_PINS

FIELDS = ["PitchStiffness", "RollStiffness", "PitchDamping", "RollDamping", "UniformAxialDamping"]
MEMBERS = ["m_" + name[0].lower() + name[1:] for name in FIELDS]

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[6])
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    assert not output.is_relative_to(args.repo.resolve()), "evidence must stay outside repo"
    output.mkdir(parents=True, exist_ok=True)
    sources = {}
    pins = {**PINS, **LOCOMOTOR_PINS}
    for relative, expected in pins.items():
        data = (args.repo / relative).read_bytes()
        assert hashlib.sha256(data).hexdigest() == expected, relative
        sources[relative] = data.decode()
    ini, _, base, cpp, header = sources.values()
    # Pin + strip comments BEFORE restricting to the actual constructor.
    active = re.sub(r"/\*.*?\*/|//[^\n]*", "", cpp, flags=re.S)
    ctor = active.split("LocomotorTemplate::LocomotorTemplate()", 1)[1].split("LocomotorTemplate::~LocomotorTemplate()", 1)[0]
    validate = active.split("void LocomotorTemplate::validate()", 1)[1].split("static void parseFrictionPerSec", 1)[0]
    assignments = []
    for field, member in zip(FIELDS, MEMBERS):
        matches = re.findall(r"\b" + member + r"\s*=\s*[^;]+;", ctor)
        assert len(matches) == 1, (member, matches)
        assignments += matches
        assert member not in validate, "no validation rewrite in this bounded slice"
        assert re.search(r'\{\s*"' + field + r'",\s*INI::parseReal,\s*NULL,\s*offsetof\(LocomotorTemplate,\s*' + member + r'\)\s*\}', active), field
        assert re.search(r"inline Real get" + field + r"\(\) const \{ return m_template->" + member + r";\}", header), field
    typedef = re.search(r"^typedef[^;]+\bReal;", base, re.M)[0]
    program = "#include <cstdio>\n#include <cstdint>\n#include <cstring>\n#include <iostream>\n#include <string>\n" + typedef + "\n"
    program += "enum { INI_INVALID_DATA=1 }; class INI { public: const char* token; const char* getNextToken(){return token;} static Real scanReal(const char*); static void parseReal(INI*,void*,void*,const void*); };\n"
    program += extract_method(ini, "scanReal") + "\n" + extract_method(ini, "parseReal")
    program += "\nstruct NarrowTemplate { " + "".join("Real " + m + ";" for m in MEMBERS) + " NarrowTemplate(){" + "".join(assignments) + "} };\n"
    program += "int main(){ static_assert(sizeof(Real)==4); std::string id; while(std::cin >> id){ NarrowTemplate t; Real* fields[]={" + ",".join("&t."+m for m in MEMBERS) + "}; std::string tokens[5]; for(int i=0;i<5;++i){std::cin >> tokens[i]; if(tokens[i]!=\"omitted\"){INI ini{tokens[i].c_str()}; INI::parseReal(&ini,nullptr,fields[i],nullptr);}} std::printf(\"%s\",id.c_str()); for(auto& s:tokens)std::printf(\" %s\",s.c_str()); for(auto f:fields){std::uint32_t b; std::memcpy(&b,f,4);std::printf(\" %08x\",b);}std::puts(\"\");}}\n"
    source = output / "suspension_original.cpp"
    source.write_text(program)
    binary = output / "suspension_original"
    command = ["c++", "-std=c++17", "-O0", "-ffp-contract=off", "-fno-fast-math", str(source), "-o", str(binary)]
    subprocess.run(command, check=True)
    cases = [("omitted", ["omitted"]*5), ("zero_positive", ["+0"]*5), ("zero_negative", ["-0"]*5)]
    for sign, token in [("positive", "+0"), ("negative", "-0")]:
        for index in range(5):
            tokens = ["omitted"]*5
            tokens[index] = token
            cases.append((f"zero_{sign}_{index}", tokens))
    cases += [("control_distinct", ["0.125", "0.25", "0.375", "0.5", "0.625"]), ("control_unclamped", ["-0.125", "1.25", "-2.5", "3.75", "-4.5"])]
    corpus = "".join(" ".join([name, *tokens])+"\n" for name,tokens in cases)
    (output / "inputs.txt").write_text(corpus)
    result = subprocess.run([str(binary)], input=corpus, text=True, capture_output=True, check=True)
    (output / "suspension_original.txt").write_text(result.stdout)
    (output / "source_pins.txt").write_text("".join(f"{sha}  {path}\n" for path,sha in pins.items()))
    (output / "extraction.txt").write_text("\n".join(assignments)+"\n"+" ".join(command)+"\n")
    print(f"Verified {len(cases)} cases, 75 original float bit outputs; narrow constructor/parser only")

if __name__ == "__main__":
    main()
