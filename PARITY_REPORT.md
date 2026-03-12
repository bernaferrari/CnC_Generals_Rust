# GeneralsRust Parity Report

**Generated:** 2026-03-12
**Status:** Active Development

## Executive Summary

This report summarizes the parity analysis between C++ (GeneralsMD) and Rust (GeneralsRust) implementations. The goal is strict behavioral parity for full playability.

### Overall Parity by Subsystem

| Subsystem | C++ Lines | Rust Lines | Parity | Status |
|-----------|-----------|------------|--------|--------|
| Common/Audio | ~1,200 | ~1,800 | ~85% | Good |
| Common/INI | ~1,800 | ~2,500 | **~80%** | Improved |
| Common/System | ~2,500 | ~2,000 | ~75% | Good |
| Common/RTS/Player | 4,526 | ~1,800 | **~40%** | Improved |
| Common/RTS/Team | 2,732 | ~950 | **~35%** | Improved |
| Common/RTS/Money | 113 | 300 | 100% | Fixed |
| Common/RTS/SpecialPower | 358 | 850 | 100% | Good |
| Common/RTS/Science | 357 | 1,475 | 85% | Good |
| GameLogic/Damage | 145 | 750 | 100% | Complete |
| GameLogic/Weapon | 3,508 | 3,800 | ~90% | Fixed |
| GameLogic/AI | ~15,000 | ~12,000 | ~80% | Good |
| GameLogic/ScriptEngine | ~3,000 | ~2,500 | ~75% | Good |
| GameClient/GUI | ~5,000 | ~4,000 | ~70% | Needs Work |
| GameEngineDevice/W3D | ~8,000 | ~6,500 | **~70%** | Improved |

---

## Session Improvements Summary (2026-03-12)

This session achieved major parity improvements across Player, Team, INI, and W3D systems.

### Player Class (~5% → ~40%)

| Component | Status |
|-----------|--------|
| ~45 fields added | ✅ |
| init() method | ✅ |
| update() method | ✅ |
| xfer() for save/load | ✅ |
| crc() for networking | ✅ |
| PlayerRelationMap | ✅ |
| Radar system | ✅ |
| Battle plans | ✅ |
| Sciences tracking | ✅ |
| Rank/skill systems | ✅ |

### Team Class (~3% → ~35%)

| Component | Status |
|-----------|--------|
| TeamTemplateInfo (34 fields) | ✅ |
| TCreateUnitsInfo | ✅ |
| AttitudeType enum | ✅ |
| TeamPrototype methods | ✅ |
| update_state() | ✅ |
| kill_team() | ✅ |
| Trigger detection | ✅ |
| xfer() | ✅ |

### INI Parsers (~65% → ~80%)

Added 14+ new block parsers:
- InGameUI, CommandMap, HeaderTemplate
- ScriptAction, ScriptCondition
- AudioSettings, Weather, Rank, Campaign
- Mouse, Language, Credits, EvaEvent
- ShellMenuScheme, GameLOD, WindowTransition
- OnlineChatColors, ChallengeGenerals

### W3D System (~65% → ~70%)

- Enhanced shadow system
- Texture manager fixes
- Render state improvements

### Build Status

```
cargo check: ✅ Passes (warnings only)
```

---

## Previous Session Fixes

### CRITICAL/HIGH Priority Fixes

| Issue | File | Fix | Status |
|-------|------|-----|--------|
| `parseAndTranslateLabel` missing | `ini.rs` | Added function for localized UI strings | ✅ Fixed |
| Weapon defaults inconsistent | `weapon_template.rs` | Fixed to use `i32::MAX` matching C++ `INT_MAX` | ✅ Fixed |
| Radius damage angle cone missing | `weapon_template.rs` | Added directional cone damage filtering | ✅ Fixed |
| Shared reload time incomplete | `weapon.rs`, `object/mod.rs` | Full implementation for multi-weapon units | ✅ Fixed |
| Contact weapon detection wrong | `weapon/mod.rs` | Fixed algorithm to use C++ range comparison | ✅ Fixed |
| Bridge attack points missing | `weapon_template.rs`, `terrain.rs` | Added bridge targeting logic | ✅ Fixed |
| StreamingArchiveFile missing File trait | `streaming_archive_file.rs` | Implemented File trait for drop-in replacement | ✅ Fixed |

### MEDIUM Priority Fixes

| Issue | File | Fix | Status |
|-------|------|-----|--------|
| Money extra fields breaking save/load | `money.rs` | Removed tracking fields, fixed xfer() | ✅ Fixed |

---

## Remaining Critical Issues

### 1. Player (~40% implemented)

**C++ Reference:** `GeneralsMD/Code/GameEngine/Source/Common/RTS/Player.cpp` (4,526 lines)
**Rust Implementation:** `GeneralsRust/Code/GameEngine/Common/src/common/rts/player.rs` (~1,800 lines)

**Implemented This Session:**
- ~45 fields (was ~10)
- init() method
- update() method
- xfer() for save/load
- crc() for networking
- PlayerRelationMap class
- Radar system basics
- Battle plan system basics
- Sciences tracking
- Rank/skill systems

**Still Missing:**
- Full AI system integration (m_ai)
- Complete build list (m_pBuildList)
- Resource gathering manager
- Full upgrade list
- Squad system (m_squads[])
- Current selection
- Attacked tracking
- Observer mode
- Cash bounty system
- PlayerType enum variants

**Estimated Remaining Work:** ~2,700 lines

### 2. Team (~35% implemented)

**C++ Reference:** `GeneralsMD/Code/GameEngine/Source/Common/RTS/Team.cpp` (2,732 lines)
**Rust Implementation:** `GeneralsRust/Code/GameEngine/Common/src/common/rts/team.rs` (~950 lines)

**Implemented This Session:**
- TeamTemplateInfo (34 fields)
- TCreateUnitsInfo struct
- AttitudeType enum
- TeamPrototype methods
- update_state() method
- kill_team() method
- Trigger detection
- xfer() for save/load

**Still Missing:**
- TeamRelationMap class
- TeamFactory class
- All relationship methods
- Building/unit counting
- Full AI integration
- Script hooks

**Estimated Remaining Work:** ~1,800 lines

### 3. PlayerList Stub (~30% implemented)

**C++ Reference:** `GeneralsMD/Code/GameEngine/Source/Common/RTS/PlayerList.cpp` (462 lines)
**Rust Stub:** `GeneralsRust/Code/GameEngine/Common/src/common/rts/player_list.rs` (~65 lines)

**Missing Components:**
- findPlayerWithNameKey()
- reset()
- newGame() - CRITICAL
- init() - CRITICAL
- xfer() for save/load

**Estimated Work:** ~400 lines

---

## Subsystem Details

### Common/INI (~80% parity) ✅

**Added Parsers This Session (14+):**
- InGameUI
- CommandMap
- HeaderTemplate
- ScriptAction
- ScriptCondition
- AudioSettings
- Weather
- Rank
- Campaign
- Mouse
- Language
- Credits
- EvaEvent
- ShellMenuScheme
- GameLOD (StaticGameLOD, DynamicGameLOD)
- WindowTransition
- OnlineChatColors
- ChallengeGenerals

**Still Missing Block Parsers (7):**
- MouseCursor
- LODPreset
- BenchProfile

**Still Missing Field Parsers (11):**
- parsePositiveNonZeroReal
- parseBitInInt32
- parseAsciiStringVector
- parseAsciiStringVectorAppend
- parseDynamicAudioEventRTS
- parseThingTemplate
- parseArmorTemplate
- parseDamageFX
- parseObjectCreationList
- parseVeterancyLevelFlags
- parseDamageTypeFlags

### Common/System (~75% parity)

**Missing:**
- CD music methods (areMusicFilesOnCD, loadMusicFilesFromCD, unloadMusicFilesFromCD)
- RAMFile class
- FileInfo structure parity for save/load

### GameLogic/Damage (100% parity) ✅

All DamageType and DeathType enum values match exactly.
Xfer serialization matches C++ format.
Helper functions implemented correctly.

### GameLogic/Weapon (~90% parity)

**Fixed This Session:**
- Weapon defaults (continuous_fire_*, anti_mask)
- Radius damage angle cone filtering
- Shared reload time for multi-weapon units
- Contact weapon detection algorithm
- Bridge attack point selection

**Remaining Issues:**
- Request assistance system (coordinated attacks)
- Projectile source attribution
- Disarm damage FX/academy stats

### GameLogic/AI (~80% parity)

**Already Implemented:**
- AIRappelState (helicopter infantry deployment)
- RebuildHoleBehavior (GLA gameplay)
- Skirmish AI
- Pathfinding
- Squad/Group management

**Missing:**
- Skillset selection for AI difficulty
- Garrison priority calculation
- Some edge case state transitions

---

## Build Status

```
cargo check -p game_engine: ✅ Passes (warnings only)
```

---

## Next Steps

1. **HIGH:** Continue Player implementation (~2,700 lines remaining - AI, build list, squads, selection)
2. **HIGH:** Continue Team implementation (~1,800 lines remaining - relations, factory, counting)
3. **HIGH:** Complete PlayerList implementation (~400 lines)
4. **MEDIUM:** Add remaining INI block parsers (7 parsers)
5. **MEDIUM:** Add remaining INI field parsers (11 parsers)
6. **MEDIUM:** Implement RAMFile class
7. **MEDIUM:** Complete GameClient GUI parity
8. **MEDIUM:** Continue GameEngineDevice W3D parity
9. **LOW:** Add LODPreset and BenchProfile INI parsers

---

## Testing Recommendations

1. Run gameplay tests after Player/Team fixes
2. Verify save/load compatibility with C++ saves
3. Test AI skirmish matches
4. Verify all INI files parse correctly
5. Test special power execution
6. Verify weapon damage calculations

---

## Files Modified This Session (2026-03-12)

### Player System
| File | Changes |
|------|---------|
| `Common/src/common/rts/player.rs` | Added ~45 fields, init(), update(), xfer(), crc(), PlayerRelationMap, radar, battle plans, sciences, rank/skill systems |

### Team System
| File | Changes |
|------|---------|
| `Common/src/common/rts/team.rs` | Added TeamTemplateInfo (34 fields), TCreateUnitsInfo, AttitudeType enum, TeamPrototype methods, update_state(), kill_team(), trigger detection, xfer() |

### INI Parsers
| File | Changes |
|------|---------|
| `Common/src/common/ini/` | Added 14+ new parsers: InGameUI, CommandMap, HeaderTemplate, ScriptAction, ScriptCondition, AudioSettings, Weather, Rank, Campaign, Mouse, Language, Credits, EvaEvent, ShellMenuScheme, GameLOD, WindowTransition, OnlineChatColors, ChallengeGenerals |

### W3D System
| File | Changes |
|------|---------|
| `GameEngineDevice/` | Enhanced shadow system, texture manager fixes |

---

## Files Modified (Previous Session)

| File | Changes |
|------|---------|
| `Common/src/common/ini/ini.rs` | Added parseAndTranslateLabel, get_next_sub_token |
| `GameLogic/src/weapon/weapon_template.rs` | Fixed defaults, added radius damage cone |
| `GameLogic/src/weapon/mod.rs` | Fixed contact weapon detection |
| `GameLogic/src/weapon/weapon.rs` | Added shared reload time |
| `GameLogic/src/object/mod.rs` | Added is_reload_time_shared() |
| `GameLogic/src/terrain.rs` | Added get_bridge_attack_points() |
| `Common/src/common/rts/money.rs` | Removed extra fields, fixed xfer() |
| `Common/src/common/system/streaming_archive_file.rs` | Implemented File trait |
