# Common/INI Subsystem Parity Analysis Report

## Overview

This report compares the C++ INI subsystem (`GeneralsMD/Code/GameEngine/Source/Common/INI/`) with the Rust port (`GeneralsRust/Code/GameEngine/Common/src/common/ini/`).

**Analysis Date:** 2026-03-12
**C++ Files:** 29 files
**Rust Files:** 33 files (28 implementation + 5 supporting)

---

## File Mapping Summary

| C++ File | Rust File | Status |
|----------|-----------|--------|
| INI.cpp | ini.rs | ✅ PORTED |
| INIAiData.cpp | ini_ai_data.rs | ✅ PORTED |
| INIAnimation.cpp | ini_animation.rs | ✅ PORTED |
| INIAudioEventInfo.cpp | ini_audio_event_info.rs | ✅ PORTED |
| INICommandButton.cpp | ini_command_button.rs | ✅ PORTED |
| INICommandSet.cpp | ini_command_set.rs | ✅ PORTED |
| INIControlBarScheme.cpp | ini_control_bar_scheme.rs | ✅ PORTED |
| INICrate.cpp | ini_crate.rs | ✅ PORTED |
| INIDamageFX.cpp | ini_damage_fx.rs | ✅ PORTED |
| INIDrawGroupInfo.cpp | ini_draw_group_info.rs | ✅ PORTED |
| INIGameData.cpp | ini_game_data.rs | ✅ PORTED |
| INIMapCache.cpp | ini_map_cache.rs | ✅ PORTED |
| INIMapData.cpp | ini_map_data.rs | ✅ PORTED |
| INIMappedImage.cpp | ini_mapped_image.rs | ✅ PORTED |
| INIMiscAudio.cpp | ini_misc_audio.rs | ✅ PORTED |
| INIModel.cpp | ini_model.rs | ✅ PORTED |
| INIMultiplayer.cpp | ini_multiplayer.rs | ✅ PORTED |
| INIObject.cpp | ini_object.rs | ✅ PORTED |
| INIParticleSys.cpp | ini_particle_sys.rs | ✅ PORTED |
| INISpecialPower.cpp | ini_special_power.rs | ✅ PORTED |
| INITerrain.cpp | ini_terrain.rs | ✅ PORTED |
| INITerrainBridge.cpp | ini_terrain_bridge.rs | ✅ PORTED |
| INITerrainRoad.cpp | ini_terrain_road.rs | ✅ PORTED |
| INIUpgrade.cpp | ini_upgrade.rs | ✅ PORTED |
| INIVideo.cpp | ini_video.rs | ✅ PORTED |
| INIWater.cpp | ini_water.rs | ✅ PORTED |
| INIWeapon.cpp | ini_weapon.rs | ✅ PORTED |
| INIWebpageURL.cpp | ini_webpage_url.rs | ✅ PORTED |

---

## CRITICAL Issues (Must Fix)

### 1. INI.cpp - Missing Xfer/CRS Support
**Severity: CRITICAL**
**C++ Lines:** 42-45, 1432-1436

The C++ implementation supports `Xfer*` for CRC/checksum verification during INI loading:
```cpp
static Xfer *s_xfer = NULL;
// ...
if (s_xfer) {
    s_xfer->xferUser(m_buffer, sizeof(char) * strlen(m_buffer));
}
```

**Rust Status:** NOT IMPLEMENTED
**Impact:** Save/load integrity checking will fail; network sync verification will be broken.

---

### 2. INI.cpp - Missing `parseAndTranslateLabel`
**Severity: CRITICAL**
**C++ Lines:** 951-963

```cpp
void INI::parseAndTranslateLabel(INI* ini, void*, void *store, const void*) {
    const char *token = ini->getNextToken();
    UnicodeString translated = TheGameText->fetch(token);
    if(translated.isEmpty())
        throw INI_INVALID_DATA;
    UnicodeString *theString = (UnicodeString *)store;
    theString->set(translated.str());
}
```

**Rust Status:** NOT IMPLEMENTED
**Impact:** INI fields that reference localized strings will fail to parse correctly.

---

### 3. INI.cpp - Missing `parseSoundsList`
**Severity: CRITICAL**
**C++ Lines:** (referenced in AudioEventInfo field parse table)

The C++ uses a custom `INI::parseSoundsList` for parsing sound lists.

**Rust Status:** Partially implemented in `ini_audio_event_info.rs` as `parse_sounds_list`
**Impact:** Minor - the Rust version exists but may have different behavior.

---

## HIGH Severity Issues

### 4. INI.cpp - Missing Block Parsers
**Severity: HIGH**
**C++ Lines:** 58-129

The C++ `theTypeTable[]` contains these block types that are `parse_passthrough_block` in Rust:

| Block Type | C++ Parser | Rust Status |
|------------|------------|-------------|
| AudioSettings | parseAudioSettingsDefinition | PASSTHROUGH |
| Campaign | parseCampaignDefinition | PASSTHROUGH |
| ChallengeGenerals | parseChallengeModeDefinition | PASSTHROUGH |
| CommandMap | parseMetaMapDefinition | PASSTHROUGH |
| Credits | parseCredits | PASSTHROUGH |
| EvaEvent | parseEvaEvent | PASSTHROUGH |
| HeaderTemplate | parseHeaderTemplateDefinition | PASSTHROUGH |
| InGameUI | parseInGameUIDefinition | PASSTHROUGH |
| Language | parseLanguageDefinition | PASSTHROUGH |
| Mouse | parseMouseDefinition | PASSTHROUGH |
| MouseCursor | parseMouseCursorDefinition | PASSTHROUGH |
| OnlineChatColors | parseOnlineChatColorDefinition | PASSTHROUGH |
| Rank | parseRankDefinition | PASSTHROUGH |
| ShellMenuScheme | parseShellMenuSchemeDefinition | PASSTHROUGH |
| StaticGameLOD | parseStaticGameLODDefinition | PASSTHROUGH |
| DynamicGameLOD | parseDynamicGameLODDefinition | PASSTHROUGH |
| LODPreset | parseLODPreset | PASSTHROUGH |
| BenchProfile | parseBenchProfile | PASSTHROUGH |
| Weather | parseWeatherDefinition | MISSING |
| WindowTransition | parseWindowTransitions | PASSTHROUGH |
| BeaconButtonDisabled | (not in C++) | PASSTHROUGH |
| BuddyButtonDisabled | (not in C++) | PASSTHROUGH |
| ButtonSet | (not in C++) | PASSTHROUGH |
| OptionsButtonDisabled | (not in C++) | PASSTHROUGH |

**Impact:** These features will not work correctly - settings won't be applied, campaigns won't load, etc.

---

### 5. INI.cpp - Missing Field Parsers
**Severity: HIGH**

The following static field parsers from C++ are missing or incomplete in Rust:

| Parser | C++ Lines | Rust Status |
|--------|-----------|-------------|
| parsePositiveNonZeroReal | 509-517 | MISSING |
| parseBitInInt32 | 543-551 | MISSING |
| parseAsciiStringVector | 588-595 | MISSING |
| parseAsciiStringVectorAppend | 600-608 | MISSING |
| parseDynamicAudioEventRTS | 834-852 | MISSING |
| parseThingTemplate | 876-896 | MISSING |
| parseArmorTemplate | 901-918 | MISSING |
| parseDamageFX | 973-990 | MISSING |
| parseObjectCreationList | 998-1014 | MISSING |
| parseVeterancyLevelFlags | (header) | MISSING |
| parseDamageTypeFlags | (header) | MISSING |
| parseDeathTypeFlags | (header) | MISSING |

---

### 6. INIControlBarScheme.cpp - Field Mismatch
**Severity: HIGH**
**C++ File:** Lines 1-91

The C++ `ControlBarScheme` uses `TheControlBar->getControlBarSchemeManager()->getFieldParse()` which is defined elsewhere. The Rust implementation has a different field structure.

**Missing fields that should be in C++ ControlBarScheme:**
- Image-based fields (all buttons use image references)
- Marker positions
- String references for UI text

**Rust Issue:** The Rust version uses placeholder RGBA color fields instead of the actual image-based button definitions.

---

## MEDIUM Severity Issues

### 7. INIAudioEventInfo.cpp - Type Mismatch
**Severity: MEDIUM**
**C++ Lines:** 33-35, 52-54

The C++ uses `TheAudio->newAudioEventInfo()` which returns an `AudioEventInfo*` that is registered with the audio system. The Rust creates standalone `AudioEventInfo` structs.

**Issue:** The Rust version doesn't integrate with the audio manager in the same way.

---

### 8. INI.cpp - Async vs Sync Loading
**Severity: MEDIUM**
**C++ Lines:** 123-168

C++ `loadDirectory` uses synchronous file I/O:
```cpp
TheFileSystem->getFileListInDirectory(dirName, "*.ini", filenameList, TRUE);
```

Rust uses async tokio:
```rust
pub async fn load_directory<P: AsRef<Path>>(...)
```

**Issue:** Different I/O models may cause timing differences in game initialization.

---

### 9. INI.cpp - String Handling Differences
**Severity: MEDIUM**
**C++ Lines:** 562-624

The C++ `getNextAsciiString()` and `getNextQuotedAsciiString()` handle quoted strings with specific edge cases for:
- Empty quotes `""`
- Quotes with spaces `"foo bar"`
- Quotes followed by additional tokens

The Rust version may not handle all these edge cases identically.

---

### 10. INIAiData.cpp - Delegation
**Severity: MEDIUM**
**C++ Lines:** 33-35

C++ delegates to `AI::parseAiDataDefinition(ini)`:
```cpp
void INI::parseAIDataDefinition(INI* ini) {
    AI::parseAiDataDefinition(ini);
}
```

The Rust version implements parsing directly in `ini_ai_data.rs`. This is structurally different but functionally equivalent.

---

## LOW Severity Issues

### 11. Error Handling Differences
**Severity: LOW**

C++ uses exceptions (`throw INI_INVALID_DATA`), Rust uses `Result<T, INIError>`.

The error values map correctly, but error propagation differs.

---

### 12. Buffer Size Constants
**Severity: LOW**
**C++ Lines:** 76 (INI.h)

```cpp
INI_MAX_CHARS_PER_LINE = 1028
INI_READ_BUFFER = 8192
```

Rust defines identical constants - this is correctly ported.

---

### 13. Line Number Tracking
**Severity: LOW**

C++ increments line number before parsing, Rust increments after some operations. This may cause off-by-one errors in error messages.

---

## Missing Functionality Summary

### Completely Missing (Must Implement):
1. `Xfer`/CRC integration for INI loading
2. `parseAndTranslateLabel` for localized strings
3. `parseWeatherDefinition` block parser
4. `parsePositiveNonZeroReal` field parser
5. `parseBitInInt32` field parser
6. `parseDynamicAudioEventRTS` field parser
7. `parseVeterancyLevelFlags` field parser
8. `parseDamageTypeFlags` field parser
9. `parseDeathTypeFlags` field parser

### Passthrough/Missing Parsers (Should Implement):
1. `AudioSettings` block
2. `Campaign` block
3. `ChallengeGenerals` block
4. `Credits` block
5. `EvaEvent` block
6. `HeaderTemplate` block
7. `InGameUI` block
8. `Language` block
9. `Mouse` block
10. `MouseCursor` block
11. `OnlineChatColors` block
12. `Rank` block
13. `ShellMenuScheme` block
14. `StaticGameLOD` block
15. `DynamicGameLOD` block
16. `LODPreset` block
17. `BenchProfile` block
18. `WindowTransition` block
19. `CommandMap` block

---

## Recommendations

### Immediate Action Required:
1. **Implement `parseAndTranslateLabel`** - Required for all localized UI strings
2. **Add Xfer/CRC support** - Required for save/load integrity
3. **Implement missing field parsers** - `parsePositiveNonZeroReal`, `parseBitInInt32`, etc.

### Short Term:
1. Implement `Weather` block parser
2. Implement `Campaign` block parser
3. Implement `InGameUI` block parser
4. Implement `Rank` block parser

### Long Term:
1. Implement remaining passthrough blocks
2. Verify edge cases in string parsing match C++ exactly
3. Add comprehensive tests comparing C++ and Rust parsing of actual game INI files

---

## Testing Recommendations

1. **Unit Tests:** Compare Rust parser output against known C++ parsed values
2. **Integration Tests:** Load actual game INI files and compare resulting data structures
3. **Edge Case Tests:** Test quoted strings, empty values, special characters
4. **Performance Tests:** Ensure Rust parsing is not significantly slower than C++

---

## Appendix: Detailed Function Comparison

### INI Class Methods (ini.rs vs INI.cpp)

| C++ Method | Rust Equivalent | Notes |
|------------|-----------------|-------|
| `load()` | `load()` | ✅ |
| `loadDirectory()` | `load_directory()` | ⚠️ async vs sync |
| `readLine()` | `read_line()` | ✅ |
| `getNextToken()` | `get_next_token()` / `get_next_value_token()` | ⚠️ Different API |
| `getNextTokenOrNull()` | Partial via Option returns | ✅ |
| `getNextSubToken()` | Missing direct equivalent | ❌ |
| `getNextAsciiString()` | `parse_ascii_string()` | ⚠️ Different edge cases |
| `getNextQuotedAsciiString()` | `parse_quoted_ascii_string()` | ⚠️ Different edge cases |
| `initFromINI()` | `init_from_ini_with_fields()` | ✅ |
| `initFromINIMulti()` | Missing | ❌ |
| `scanInt()` | `parse_int()` | ✅ |
| `scanUnsignedInt()` | `parse_unsigned_int()` | ✅ |
| `scanReal()` | `parse_real()` | ✅ |
| `scanBool()` | `parse_bool()` | ⚠️ Accepts more values |
| `scanPercentToReal()` | `parse_percent_to_real()` | ✅ |
| `scanScience()` | Missing direct equivalent | ❌ |
| `scanIndexList()` | `parse_index_list()` | ✅ |
| `scanLookupList()` | `parse_lookup_list()` | ✅ |

### Field Parsers Comparison

| C++ Parser | Rust Status | Location |
|------------|-------------|----------|
| `parseUnsignedByte` | ✅ `parse_unsigned_byte` | ini.rs |
| `parseShort` | ✅ `parse_short` | ini.rs |
| `parseUnsignedShort` | ✅ `parse_unsigned_short` | ini.rs |
| `parseInt` | ✅ `parse_int` | ini.rs |
| `parseUnsignedInt` | ✅ `parse_unsigned_int` | ini.rs |
| `parseReal` | ✅ `parse_real` | ini.rs |
| `parsePositiveNonZeroReal` | ❌ MISSING | - |
| `parseBool` | ✅ `parse_bool` | ini.rs |
| `parseBitInInt32` | ❌ MISSING | - |
| `parseAsciiString` | ✅ `parse_ascii_string` | ini.rs |
| `parseQuotedAsciiString` | ✅ `parse_ascii_string` | ini.rs (handles quotes) |
| `parseAsciiStringVector` | ❌ MISSING | - |
| `parseAsciiStringVectorAppend` | ❌ MISSING | - |
| `parseAndTranslateLabel` | ❌ MISSING | - |
| `parseMappedImage` | ⚠️ Partial | Various modules |
| `parseAnim2DTemplate` | ⚠️ Partial | ini_animation.rs |
| `parsePercentToReal` | ✅ `parse_percent_to_real` | ini.rs |
| `parseRGBColor` | ✅ `parse_rgb_color` | ini.rs |
| `parseRGBAColorInt` | ⚠️ Partial | Various modules |
| `parseColorInt` | ⚠️ `parse_color_int` | ini.rs |
| `parseCoord3D` | ✅ `parse_coord_3d` | ini.rs |
| `parseCoord2D` | ✅ `parse_coord_2d` | ini.rs |
| `parseICoord2D` | ⚠️ Used but not exposed | Various modules |
| `parseDynamicAudioEventRTS` | ❌ MISSING | - |
| `parseAudioEventRTS` | ⚠️ Partial | ini_misc_audio.rs |
| `parseFXList` | ⚠️ Partial | ini_fx_list.rs |
| `parseParticleSystemTemplate` | ⚠️ Partial | ini_particle_sys.rs |
| `parseObjectCreationList` | ⚠️ Stored raw | ini.rs |
| `parseSpecialPowerTemplate` | ⚠️ Partial | ini_special_power.rs |
| `parseUpgradeTemplate` | ⚠️ Partial | ini_upgrade.rs |
| `parseScience` | ⚠️ Partial | ini_science.rs |
| `parseScienceVector` | ❌ MISSING | - |
| `parseGameClientRandomVariable` | ❌ MISSING | - |
| `parseBitString8` | ⚠️ Via `parse_bit_string_32` | ini.rs |
| `parseBitString32` | ✅ `parse_bit_string_32` | ini.rs |
| `parseByteSizedIndexList` | ❌ MISSING | - |
| `parseIndexList` | ✅ `parse_index_list` | ini.rs |
| `parseLookupList` | ✅ `parse_lookup_list` | ini.rs |
| `parseThingTemplate` | ❌ MISSING | - |
| `parseArmorTemplate` | ⚠️ Stored raw | ini.rs |
| `parseDamageFX` | ⚠️ Partial | ini_damage_fx.rs |
| `parseWeaponTemplate` | ⚠️ Partial | ini_weapon.rs |
| `parseDurationReal` | ✅ `parse_duration_real` | ini.rs |
| `parseDurationUnsignedInt` | ✅ `parse_duration_unsigned_int` | ini.rs |
| `parseDurationUnsignedShort` | ❌ MISSING | - |
| `parseVelocityReal` | ✅ `parse_velocity_real` | ini.rs |
| `parseAccelerationReal` | ❌ MISSING | - |
| `parseAngleReal` | ✅ `parse_angle_real` | ini.rs |
| `parseAngularVelocityReal` | ✅ `parse_angular_velocity_real` | ini.rs |
| `parseDamageTypeFlags` | ❌ MISSING | - |
| `parseDeathTypeFlags` | ❌ MISSING | - |
| `parseVeterancyLevelFlags` | ❌ MISSING | - |
| `parseSoundsList` | ⚠️ Partial | ini_audio_event_info.rs |
