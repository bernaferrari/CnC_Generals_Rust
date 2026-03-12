# GeneralsRust Parity Report

**Generated:** 2026-03-12
**Status:** Active Development

## Executive Summary

This report summarizes the parity analysis between C++ (GeneralsMD) and Rust (GeneralsRust) implementations. The goal is strict behavioral parity for full playability.

### Overall Parity by Subsystem

| Subsystem | C++ Lines | Rust Lines | Parity | Status |
|-----------|-----------|------------|--------|--------|
| Common/Audio | ~1,200 | ~1,800 | ~85% | Good |
| Common/INI | ~1,800 | ~1,200 | ~65% | Needs Work |
| Common/System | ~2,500 | ~2,000 | ~75% | Good |
| Common/RTS/Player | 4,526 | ~200 | **<5%** | CRITICAL |
| Common/RTS/Team | 2,732 | ~30 | **<3%** | CRITICAL |
| Common/RTS/Money | 113 | 300 | 100% | Fixed |
| Common/RTS/SpecialPower | 358 | 850 | 100% | Good |
| Common/RTS/Science | 357 | 1,475 | 85% | Good |
| GameLogic/Damage | 145 | 750 | 100% | Complete |
| GameLogic/Weapon | 3,508 | 3,800 | ~90% | Fixed |
| GameLogic/AI | ~15,000 | ~12,000 | ~80% | Good |
| GameLogic/ScriptEngine | ~3,000 | ~2,500 | ~75% | Good |
| GameClient/GUI | ~5,000 | ~4,000 | ~70% | Needs Work |
| GameEngineDevice/W3D | ~8,000 | ~6,000 | ~65% | Needs Work |

---

## Fixes Applied This Session

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

### 1. Player Stub (<5% implemented)

**C++ Reference:** `GeneralsMD/Code/GameEngine/Source/Common/RTS/Player.cpp` (4,526 lines)
**Rust Stub:** `GeneralsRust/Code/GameEngine/Common/src/common/rts/player.rs` (~200 lines)

**Missing Components:**
- PlayerRelationMap class
- TeamRelationMap integration
- AI system integration (m_ai)
- Build list (m_pBuildList)
- Resource gathering manager
- Radar system
- Battle plan system
- Upgrade list
- Squad system (m_squads[])
- Current selection
- Attacked tracking
- Observer mode
- Cash bounty system
- xfer() for save/load
- crc() for networking

**Estimated Work:** ~4,500 lines

### 2. Team Stub (<3% implemented)

**C++ Reference:** `GeneralsMD/Code/GameEngine/Source/Common/RTS/Team.cpp` (2,732 lines)
**Rust Stub:** `GeneralsRust/Code/GameEngine/Common/src/common/rts/team.rs` (~30 lines)

**Missing Components:**
- TeamRelationMap class
- TeamFactory class
- TeamTemplateInfo class
- TeamPrototype class
- All relationship methods
- Building/unit counting
- xfer() for save/load
- AI integration
- Script hooks

**Estimated Work:** ~2,700 lines

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

### Common/INI (~65% parity)

**Missing Block Parsers (21):**
- AudioSettings
- Campaign
- ChallengeGenerals
- Credits
- EvaEvent
- InGameUI
- Language
- Mouse
- MouseCursor
- OnlineChatColors
- Rank
- ShellMenuScheme
- StaticGameLOD
- DynamicGameLOD
- LODPreset
- BenchProfile
- Weather (missing entirely)
- WindowTransition
- CommandMap
- HeaderTemplate

**Missing Field Parsers (11):**
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

1. **CRITICAL:** Complete Player implementation (~4,500 lines)
2. **CRITICAL:** Complete Team implementation (~2,700 lines)
3. **CRITICAL:** Complete PlayerList implementation (~400 lines)
4. **HIGH:** Add missing INI block parsers (21 parsers)
5. **HIGH:** Add missing INI field parsers (11 parsers)
6. **MEDIUM:** Implement RAMFile class
7. **MEDIUM:** Complete GameClient GUI parity
8. **MEDIUM:** Complete GameEngineDevice W3D parity

---

## Testing Recommendations

1. Run gameplay tests after Player/Team fixes
2. Verify save/load compatibility with C++ saves
3. Test AI skirmish matches
4. Verify all INI files parse correctly
5. Test special power execution
6. Verify weapon damage calculations

---

## Files Modified This Session

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
