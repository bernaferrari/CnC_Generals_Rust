#!/usr/bin/env python3
"""Execute original Money.cpp and the Player::init cash slice.

Services are bounded compile adapters; this compares admission cash arithmetic,
not a full PlayerList, rendered match, audio delivery or original save stream.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import subprocess
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[4])
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    repo = args.repo_root.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    scratch = output / "compile"
    scratch.mkdir(exist_ok=True)
    base = repo / "GeneralsMD/Code/GameEngine"
    money_cpp = base / "Source/Common/RTS/Money.cpp"
    money_h = base / "Include/Common/Money.h"
    player_cpp = base / "Source/Common/RTS/Player.cpp"
    source = player_cpp.read_text()
    start = source.index("m_money = *pt->getMoney();")
    stop = source.index("m_playerDisplayName.clear();", start)
    cash_slice = source[start:stop]
    assert "TheGameInfo->getStartingCash()" in cash_slice
    assert "TheGlobalData->m_defaultStartingCash" in cash_slice
    files = {
        "Lib/BaseType.h": "#pragma once\n#include <cstdint>\nusing UnsignedInt=uint32_t; using Int=int32_t; using Bool=bool; using XferVersion=uint8_t; constexpr bool TRUE=true; class INI; class Xfer;\n",
        "Common/Debug.h": "#pragma once\n",
        "Common/Snapshot.h": "#pragma once\nclass Xfer; class Snapshot { protected: virtual void crc(Xfer*)=0; virtual void xfer(Xfer*)=0; virtual void loadPostProcess()=0; };\n",
        "Common/Money.h": money_h.read_text(),
        "Common/Xfer.h": "#pragma once\n#include \"Lib/BaseType.h\"\nclass Xfer { public: void xferVersion(XferVersion*,XferVersion){} void xferUnsignedInt(UnsignedInt*){} };\n",
        "services.h": """#pragma once
#include "Lib/BaseType.h"
class INI { public: UnsignedInt value; static void parseUnsignedInt(INI* ini,void*,void* store,const void*) { *static_cast<UnsignedInt*>(store)=ini->value; } };
struct AudioEventRTS { Int player=0; void setPlayerIndex(Int id){player=id;} };
struct MiscAudio { AudioEventRTS m_moneyWithdrawSound, m_moneyDepositSound; };
struct Audio { MiscAudio misc; Int deposits=0,last_player=-1; MiscAudio* getMiscAudio(){return &misc;} void addAudioEvent(AudioEventRTS* e){++deposits;last_player=e->player;} };
struct Academy { Int calls=0; void recordIncome(){++calls;} };
struct Player { Academy academy; Academy* getAcademyStats(){return &academy;} };
struct PlayerList { Player player; Int last_player=-1; Player* getNthPlayer(Int id){last_player=id;return &player;} };
extern Audio* TheAudio; extern PlayerList* ThePlayerList;
""",
        "PreRTS.h": "#include \"services.h\"\n",
    }
    for name in ["GameAudio", "MiscAudio", "Player", "PlayerList"]:
        files[f"Common/{name}.h"] = '#include "services.h"\n'
    for name, text in files.items():
        path = scratch / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
    harness = """#include "services.h"
#include "Common/Money.h"
#include <iostream>
Audio audio; PlayerList players; Audio* TheAudio=&audio; PlayerList* ThePlayerList=&players;
Money money(UnsignedInt value){ Money result; INI ini{value}; Money::parseMoneyAmount(&ini,nullptr,&result,nullptr); return result; }
struct Template { Money cash; Int handicap=0; const Money* getMoney()const{return &cash;} const Int* getHandicap()const{return &handicap;} };
struct Info { Money cash; Money getStartingCash()const{return cash;} };
struct GlobalData { Money m_defaultStartingCash; };
Info* TheGameInfo; GlobalData* TheGlobalData;
struct Admission { Money m_money; Int m_handicap=0; Int getPlayerIndex()const{return 7;} void init(Template* pt) {
""" + cash_slice + """
} };
int main(){
    struct Case { UnsignedInt cash, template_cash; Int deposit; };
    for(auto c: {Case{10000,0,0},Case{12500,0,700},Case{0,0,0},Case{12500,4500,2500},Case{12500,0,-1},Case{UINT32_MAX,0,2}}){
        audio.deposits=0; audio.last_player=-1; players.player.academy.calls=0;
        Info info{money(c.cash)}; GlobalData global{money(10000)}; TheGameInfo=&info; TheGlobalData=&global;
        Template pt{money(c.template_cash)}; Admission admission; admission.init(&pt);
        admission.m_money.deposit(c.deposit);
        std::cout<<c.cash<<","<<c.template_cash<<","<<c.deposit<<","<<admission.m_money.countMoney()<<","<<audio.deposits<<","<<audio.last_player<<","<<players.player.academy.calls<<"\\n";
    }
}
"""
    driver = scratch / "cash.cpp"
    driver.write_text(harness)
    executable = output / "cpp-player-cash"
    command = ["c++", "-std=c++17", "-O2", "-Wall", "-Wextra", "-Wpedantic", "-fsanitize=undefined", "-fno-sanitize-recover=all", f"-I{scratch}", str(driver), str(money_cpp), "-o", str(executable)]
    subprocess.run(command, check=True)
    result = subprocess.check_output([str(executable)])
    assert len(result.splitlines()) == 6
    (output / "cash.csv").write_bytes(result)
    receipt = {"command": command, "compiler": subprocess.check_output(["c++", "--version"], text=True).splitlines()[0], "runner_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), "sources": {str(p.relative_to(repo)): hashlib.sha256(p.read_bytes()).hexdigest() for p in [money_cpp,money_h,player_cpp]}, "player_cash_range": [source[:start].count("\n")+1,source[:stop].count("\n")+1], "executable_sha256": hashlib.sha256(executable.read_bytes()).hexdigest(), "driver_sha256": hashlib.sha256(driver.read_bytes()).hexdigest(), "output_sha256": hashlib.sha256(result).hexdigest(), "scope": "Executed original Money.cpp and extracted Player::init cash statements; compile adapters only observe service calls. Not whole-game equivalence."}
    (output / "receipt.json").write_text(json.dumps(receipt, indent=2)+"\n")
    print(result.decode(), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
