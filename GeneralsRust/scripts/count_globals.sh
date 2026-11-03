#!/usr/bin/env bash
# Count process-wide mutable statics per crate area.
# Counts: `static mut`, `static` with Mutex|RwLock|Atomic*|RefCell|Cell
# (including those nested under OnceLock/Lazy/LazyLock/OnceCell),
# opaque *Slot/*Bag wrappers whose same-file struct body holds those types,
# and `thread_local!`. Excludes tests/, examples/, benches/, *_tests.rs,
# tests.rs, and items under #[cfg(test)] modules/attributes.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
python3 - "$ROOT" "$@" <<'PY'
import os, re, sys, json, collections

root = sys.argv[1]
want_json = "--json" in sys.argv[1:]
compare = None
for i, a in enumerate(sys.argv):
    if a == "--compare" and i + 1 < len(sys.argv):
        compare = sys.argv[i + 1]

pat_static = re.compile(
    r"(?m)^[ \t]*(?:pub(?:\([^)]*\))?[ \t]+)?static[ \t]+(?:mut[ \t]+)?(\w+)[ \t]*:[ \t]*([^;=]+)"
)
pat_tls = re.compile(r"(?m)^[ \t]*thread_local!\s*\{")
mut_ty = re.compile(r"\b(Mutex|RwLock|Atomic\w+|RefCell|Cell)\b")
# Opaque wrappers (FooSlot / FooBag) hide interior Mutex/Atomic from the type
# line; treat them as mutable when the same-file struct body contains mut_ty.
opaque_name = re.compile(r"\b([A-Za-z_][A-Za-z0-9_]*(?:Slot|Bag))\b")


def struct_body_is_mutable(text, type_name):
    """True if `struct TypeName` / `pub struct TypeName` body mentions mut_ty."""
    # Strip generics/wrappers: LazyLock<FooSlot> -> FooSlot
    names = opaque_name.findall(type_name)
    if not names:
        return False
    for name in names:
        for m in re.finditer(
            rf"(?m)^[ \t]*(?:pub(?:\([^)]*\))?[ \t]+)?struct[ \t]+{re.escape(name)}\b[^{{]*\{{",
            text,
        ):
            start = m.end() - 1
            depth = 0
            i = start
            while i < len(text):
                if text[i] == "{":
                    depth += 1
                elif text[i] == "}":
                    depth -= 1
                    if depth == 0:
                        body = text[start : i + 1]
                        if mut_ty.search(body):
                            return True
                        break
                i += 1
    return False


def cfg_test_depths(text):
    """Byte offsets that sit inside a #[cfg(test)] module body."""
    depths = set()
    depth = 0
    test_depth = None
    i = 0
    n = len(text)
    while i < n:
        if text.startswith("#[cfg(test)]", i):
            j = i + len("#[cfg(test)]")
            while j < n and text[j] in " \t\r\n":
                j += 1
            # Attribute on a module: track its body. Attribute on an item: mark
            # the next item span roughly via following '{'/';' .
            if text.startswith("mod ", j) or text.startswith("pub mod ", j):
                k = text.find("{", j)
                if k != -1 and k < n:
                    # enter module at k
                    pass
                # find opening brace of the mod
                k = j
                while k < n and text[k] != "{":
                    if text[k] == ";":  # mod foo;
                        break
                    k += 1
                if k < n and text[k] == "{":
                    test_depth = depth + 1
                    i = k
                    depth += 1
                    i += 1
                    continue
            else:
                # cfg(test) on a non-mod item — skip until next top-level-ish end
                depths.add(j)  # marker; handled per-match via lookback
            i = j
            continue
        ch = text[i]
        if ch == "{":
            depth += 1
        elif ch == "}":
            if test_depth is not None and depth == test_depth:
                test_depth = None
            depth -= 1
        if test_depth is not None and depth >= test_depth:
            depths.add(i)
        i += 1
    return depths

def item_has_cfg_test_attr(text, pos):
    line = text.rfind("\n", 0, pos) + 1
    window = text[max(0, line - 240):line]
    for raw in reversed(window.splitlines()):
        s = raw.strip()
        if not s:
            continue
        if s.startswith("#[cfg(test)]"):
            return True
        if s.startswith("#["):
            continue
        break
    return False

def inside_cfg_test_thread_local(text, pos):
    """True when pos sits in a thread_local! block preceded by #[cfg(test)]."""
    # Find the nearest thread_local! before pos whose braces still enclose pos.
    idx = text.rfind("thread_local!", 0, pos)
    while idx != -1:
        brace = text.find("{", idx)
        if brace == -1 or brace > pos:
            idx = text.rfind("thread_local!", 0, idx)
            continue
        depth = 0
        i = brace
        end = None
        while i < len(text):
            if text[i] == "{":
                depth += 1
            elif text[i] == "}":
                depth -= 1
                if depth == 0:
                    end = i
                    break
            i += 1
        if end is not None and brace < pos <= end:
            return item_has_cfg_test_attr(text, idx)
        idx = text.rfind("thread_local!", 0, idx)
    return False

roots = {
    "GameClient": "Code/GameEngine/GameClient",
    "GameLogic": "Code/GameEngine/GameLogic",
    "Common": "Code/GameEngine/Common",
    "WWVegas": "Code/Libraries/Source/WWVegas",
    "Main": "Code/Main",
}
skip_dir = re.compile(r"(?:^|/)(?:tests|examples|benches|target)(?:/|$)")

counts = collections.OrderedDict((k, 0) for k in roots)
details = collections.defaultdict(list)

for label, rel in roots.items():
    base = os.path.join(root, rel)
    if not os.path.isdir(base):
        continue
    for dirpath, dirnames, fns in os.walk(base):
        # prune skipped dirs
        dirnames[:] = [d for d in dirnames if not skip_dir.search(d)]
        rel_dir = os.path.relpath(dirpath, root).replace("\\", "/")
        if skip_dir.search(rel_dir):
            continue
        for fn in fns:
            if not fn.endswith(".rs"):
                continue
            if fn.endswith("_tests.rs") or fn == "tests.rs":
                continue
            path = os.path.join(dirpath, fn)
            text = open(path, encoding="utf-8", errors="ignore").read()
            test_body = cfg_test_depths(text)
            for m in pat_tls.finditer(text):
                line = text.rfind("\n", 0, m.start()) + 1
                if text[line : line + 8].lstrip().startswith("//"):
                    continue
                if (
                    m.start() in test_body
                    or item_has_cfg_test_attr(text, m.start())
                ):
                    continue
                counts[label] += 1
                details[label].append(f"{rel_dir}/{fn}:thread_local!")
            for m in pat_static.finditer(text):
                line = text.rfind("\n", 0, m.start()) + 1
                head = text[line : m.end()]
                if head.lstrip().startswith("//"):
                    continue
                name, ty = m.group(1), m.group(2)
                is_mut = (
                    re.search(r"\bstatic\s+mut\b", head) is not None
                    or mut_ty.search(ty) is not None
                    or struct_body_is_mutable(text, ty)
                )
                if is_mut:
                    if (
                        m.start() in test_body
                        or item_has_cfg_test_attr(text, m.start())
                        or inside_cfg_test_thread_local(text, m.start())
                    ):
                        continue
                    counts[label] += 1
                    details[label].append(f"{rel_dir}/{fn}:{name}")

if want_json:
    print(json.dumps(dict(counts), indent=2, sort_keys=True))
else:
    print("Mutable process-wide statics (excl. tests/examples/benches):")
    total = 0
    for k, v in counts.items():
        print(f"  {k}: {v}")
        total += v
    print(f"  TOTAL: {total}")


# --- Slot/Bag wrapper lint (always) ---
# Opaque *Slot/*Bag types hid interior Mutex/Atomic from the counter. Existing
# wrappers are allowlisted; adding a new one fails this script (migrate onto
# owned per-instance context instead — see root AGENTS.md).
ALLOW_SLOT_BAG = {
    "AcademyTemplateContextSlot",
    "AcceleratorsSlot",
    "AiDataActiveSlot",
    "AiDataProcessLifetimeSlot",
    "Anim2DCollectionSlot",
    "ApplicationInstanceSlot",
    "ApplicationIsWindowedSlot",
    "ApplicationWindowSlot",
    "ArchiveFileSystemSlot",
    "ArchiveSystemSlot",
    "AssertHandlerSlot",
    "AssetManagerSlot",
    "AsyncPlaybackHandlePoolSlot",
    "AudioEventLocalizationLanguageSlot",
    "AudioMainWindowHandleSlot",
    "AudioSettingsSlot",
    "AuthoredDrawWeaponBoneSlot",
    "BoxDisplayMaskSlot",
    "BridgesInstalledSlot",
    "BufferSlot",
    "BuildAssistantBackendSlot",
    "BuildAssistantSlot",
    "ButtonSlot",
    "CallbackCountSlot",
    "CallsSlot",
    "CampaignStoreSlot",
    "CdManagerSlot",
    "CommandLineArgsSlot",
    "CommandListAppendFnSlot",
    "CommandSetManagerSlot",
    "CommandSystemSlot",
    "ConfigBag",
    "ControlBarSchemeManagerSlot",
    "ControlBarSlot",
    "CopyProtectionEnabledSlot",
    "CopyProtectionSlot",
    "CounterSlot",
    "CrateSystemSlot",
    "CrateTestLockSlot",
    "CrcDebugBag",
    "CreditsManagerSlot",
    "CursorInfoMapSlot",
    "DamageFxStoreBag",
    "DamageFxStoreSlot",
    "DazzleManagerSlotBag",
    "DazzleRenderingEnabledSlot",
    "DazzleTextureBag",
    "DazzleTypesBag",
    "DazzleVisibilityHandlerBag",
    "DebugCameraMoveLogsSlot",
    "DebugCameraMoveToLogsSlot",
    "DebugSystemSlot",
    "DecalIdCounterSlot",
    "DecalSystemBag",
    "DecisionsBag",
    "DefaultRemoveOnCompleteSlot",
    "DefinitionMgrLockSlot",
    "DeliveredSlot",
    "DevelopmentModeSlot",
    "DisabledColorMaskWarningsSlot",
    "DispatchedFxNuggetsSlot",
    "DrawGroupInfoSlot",
    "DriveFrameCountSlot",
    "EmitterIdCounterSlot",
    "EmptyBag",
    "EnablePerPolygonCullingSlot",
    "EngineSlot",
    "EvaEventStoreSlot",
    "ExplicitVertexColorDebugsSlot",
    "ExtraBlockParsersSlot",
    "FilesSlot",
    "FormationSlot",
    "FrameActiveSlot",
    "FrameIndexAtSubmitSlot",
    "FxBonePosRuntimeSlot",
    "FxListObjRuntimeSlot",
    "FxListStoreSlot",
    "FxListTestLockSlot",
    "GameClientFactorySlot",
    "GameEngineInstanceSlot",
    "GameInFocusSlot",
    "GameLodManagerSlot",
    "GameSlot",
    "GlobalAssetManagerBag",
    "GlobalDataSlot",
    "GlobalDataTestLockSlot",
    "GlobalLanguageSlot",
    "GlobalLocalizationSlot",
    "GlobalManagerSlot",
    "GlobalMemoryPoolFactorySlot",
    "GlobalRecorderSlot",
    "GlobalRendererSlot",
    "GlobalScaleBitsSlot",
    "HandlersSlot",
    "HeaderTemplateManagerSlot",
    "HookTestLockSlot",
    "HostBoneFxSlot",
    "HostDestroyLogLogBag",
    "HostFireIntentLogLogBag",
    "HostProductionProgressLogLogBag",
    "HostStateResidualTestEnvLockSlot",
    "HostTriggerSlot",
    "HudFeedbackChainLockSlot",
    "HulkMaxLifetimeOverrideSlot",
    "IngameUiSettingsSlot",
    "InstanceSlot",
    "IsInittedSlot",
    "IsLockedSlot",
    "LastFrameSubmitCountSlot",
    "LastOutOfFrameCountSlot",
    "LauncherShutdownRequestedSlot",
    "LightingLodCutoffBag",
    "LiveGameClientSlot",
    "LockSlot",
    "LocomotorStoreSlot",
    "LodMaxScreenSizesBag",
    "LowVertexColorWarningsSlot",
    "MainWindowSlot",
    "MapCacheSlot",
    "MapDataSlot",
    "MappedImageCollectionSlot",
    "MaterialProbeFrameCounterSlot",
    "MeshDebugIdCountSlot",
    "MessageInterceptHandlerSlot",
    "MetaMapSlot",
    "MiscAudioSlot",
    "ModelManagerSlot",
    "ModelessDialogsSlot",
    "ModuleFactorySlot",
    "MouseCursorSlot",
    "MouseSettingsSlot",
    "MssLockDepthBag",
    "MultiplayerSettingsSlot",
    "NameKeyStateBag",
    "NewestTimestampSlot",
    "NextHandleSlot",
    "NextIdSlot",
    "NextLivePopupGenerationSlot",
    "NextLogicalDecalIdSlot",
    "NextTempFileSlot",
    "ObjectDataProviderBag",
    "ObjectDefinitionRegistrySlot",
    "ObjectSlot",
    "OldestTimestampSlot",
    "OnlineChatColorsSlot",
    "ParticleDebugDisableSlot",
    "ParticleSystemLiveOverlaySlot",
    "ParticleSystemManagerSlot",
    "PendingSlot",
    "PerfGatherRegistryBag",
    "PlayerListLockSlot",
    "PlayerObjectWorldBag",
    "PlayerSlot",
    "PlayerTemplateStoreSlot",
    "PoolRegistrySlot",
    "PostLoadHooksSlot",
    "ProbeFrameCounterSlot",
    "ProgramInstanceSlot",
    "QSlot",
    "RadarSystemSlot",
    "RankInfoStoreSlot",
    "RegistryByIdBag",
    "RenderCallCountSlot",
    "RendererStorageSlot",
    "ReplaySlot",
    "ResolvedSkirmishSlot",
    "ResourceManagerSlot",
    "RetailGameDataProbeSlot",
    "RingIdCounterSlot",
    "RingManagerSlotBag",
    "RingMeshArrayBag",
    "RngLockSlot",
    "RuntimeIdCounterHooksSlot",
    "SaveLoadLifecycleHooksSlot",
    "SaveLoadManagerSlot",
    "ScienceStoreSlot",
    "SettingsBag",
    "ShaderStateBag",
    "ShatterSystemBag",
    "ShatterSystemSlot",
    "ShatterSystemSlotBag",
    "SinkCallbackCacheSlot",
    "SkirmishSlot",
    "SlotSlot",
    "SluiRosterTicksSlot",
    "SpecialPowerStoreSlot",
    "SpeechRegistrySlot",
    "StartupModelTraceCountSlot",
    "StateSlot",
    "StaticSortStateBag",
    "StatisticsBag",
    "SubsystemSlot",
    "TeamCommandSinkSlot",
    "TeamFactorySlot",
    "TeamHomeWaypointSlot",
    "TerrainRoadCollectionSlot",
    "TerrainRoadsSlot",
    "TerrainTypesSlot",
    "TestGuardSlot",
    "TestLockSlot",
    "TextureCacheBag",
    "TheAudioSlot",
    "TheDefinitionMgrSlot",
    "TheFactoryBag",
    "TheGameStateSlot",
    "TheLocalFileSystemBag",
    "TheMessageTimeSlot",
    "TheWin32MouseSlot",
    "ThumbnailManagerBag",
    "TimeSlot",
    "TotalOwnedSubmitsSlot",
    "UpgradeCenterActiveSlot",
    "UpgradeCenterProcessLifetimeSlot",
    "VersionInstanceSlot",
    "VideoPlayerSlot",
    "WarningCountSlot",
    "WeaponStoreSlot",
    "WeatherSettingSlot",
    "WebBrowserSlot",
    "WgpuWrapperInstanceSlot",
    "WindowMsgPayloadSlot",
    "WindowTransitionStoreSlot",
    "WritableGlobalDataSlot",
    "Ww3DStateBag",
    "WwAudioInstanceBag",
    "ZhRegistryBag"
}
pat_slot_struct = re.compile(
    r"(?m)^[ \t]*(?:pub(?:\([^)]*\))?[ \t]+)?struct[ \t]+([A-Za-z_][A-Za-z0-9_]*(?:Slot|Bag))\b"
)
new_wrappers = []
for label, rel in roots.items():
    base = os.path.join(root, rel)
    if not os.path.isdir(base):
        continue
    for dirpath, dirnames, fns in os.walk(base):
        dirnames[:] = [d for d in dirnames if not skip_dir.search(d)]
        rel_dir = os.path.relpath(dirpath, root).replace("\\", "/")
        if skip_dir.search(rel_dir):
            continue
        for fn in fns:
            if not fn.endswith(".rs") or fn.endswith("_tests.rs") or fn == "tests.rs":
                continue
            path = os.path.join(dirpath, fn)
            body = open(path, encoding="utf-8", errors="ignore").read()
            for m in pat_slot_struct.finditer(body):
                name = m.group(1)
                if name not in ALLOW_SLOT_BAG:
                    new_wrappers.append(f"{rel_dir}/{fn}:{name}")
if new_wrappers:
    print("FAIL: new opaque Slot/Bag wrappers (migrate to owned context, do not add):", file=sys.stderr)
    for w in new_wrappers:
        print(f"  {w}", file=sys.stderr)
    sys.exit(1)

if compare:
    base = json.load(open(compare, encoding="utf-8"))
    failed = False
    for k, v in counts.items():
        b = int(base.get(k, 0))
        if v > b:
            print(f"FAIL: {k} rose {b} -> {v}", file=sys.stderr)
            failed = True
        elif v < b and not want_json:
            print(f"note: {k} fell {b} -> {v} (update baseline)")
    if failed:
        sys.exit(1)
PY
