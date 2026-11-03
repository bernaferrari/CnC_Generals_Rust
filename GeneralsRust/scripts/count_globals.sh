#!/usr/bin/env bash
# Count process-wide mutable statics per crate area.
# Counts: `static mut`, `static` whose type has interior mutability
# (Mutex|RwLock|Atomic*|RefCell|Cell), opaque same-file wrapper structs
# (any name — Slot/Bag/Runtime/…) whose body holds those or an
# OnceLock/LazyLock/OnceCell/Lazy, OnceLock/Lazy of non-lookup payloads,
# and `thread_local!`. Excludes tests/, examples/, benches/, *_tests.rs,
# tests.rs, and items under #[cfg(test)] modules/attributes.
# Truly immutable data (consts, numeric lookup tables, Instant/Duration,
# unit init flags, fn-pointer slots) is allowed and not counted.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
python3 - "$ROOT" "$@" <<'PY'
import os, re, sys, json, collections

root = sys.argv[1]
want_json = "--json" in sys.argv[1:]
with_cfg_test = "--with-cfg-test" in sys.argv[1:]
compare = None
for i, a in enumerate(sys.argv):
    if a == "--compare" and i + 1 < len(sys.argv):
        compare = sys.argv[i + 1]

pat_static = re.compile(
    # Type runs to the initializer `=`; must not stop at `;` inside `[T; N]`.
    r"(?m)^[ \t]*(?:pub(?:\([^)]*\))?[ \t]+)?static[ \t]+(?:mut[ \t]+)?(\w+)[ \t]*:[ \t]*(.+?)[ \t]*="
)
pat_tls = re.compile(r"(?m)^[ \t]*thread_local!\s*\{")
classic_mut = re.compile(r"\b(Mutex|RwLock|Atomic\w+|RefCell|Cell)\b")
init_cell = re.compile(
    r"\b(?:std::sync::|once_cell::(?:sync|unsync)::|parking_lot::)?(?:OnceLock|LazyLock|OnceCell|Lazy)\b"
)
type_names = re.compile(r"\b([A-Z][A-Za-z0-9_]*)\b")
SKIP_TYPE_NAMES = {
    "OnceLock", "LazyLock", "OnceCell", "Lazy", "Option", "Arc", "Box", "Vec",
    "Mutex", "RwLock", "RefCell", "Cell", "Result", "HashMap", "HashSet",
    "BTreeMap", "BTreeSet", "String", "PathBuf", "Duration", "Instant",
    "Ordering", "PhantomData", "Cow", "Rc", "Weak", "Pin", "ManuallyDrop",
}
body_mut = re.compile(
    r"\b(Mutex|RwLock|Atomic\w+|RefCell|Cell|OnceLock|LazyLock|OnceCell|Lazy)\b"
)


def struct_is_unit(text, type_name):
    return (
        re.search(
            rf"(?m)^[ \t]*(?:pub(?:\([^)]*\))?[ \t]+)?struct[ \t]+{re.escape(type_name)}\s*;",
            text,
        )
        is not None
    )


def struct_body_text(text, type_name):
    for m in re.finditer(
        rf"(?m)^[ \t]*(?:pub(?:\([^)]*\))?[ \t]+)?struct[ \t]+{re.escape(type_name)}\b[^;{{]*\{{",
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
                    return text[start : i + 1]
            i += 1
    return None


def wrapper_struct_is_mutable(text, type_annotation):
    for name in type_names.findall(type_annotation):
        if name in SKIP_TYPE_NAMES:
            continue
        body = struct_body_text(text, name)
        if body and body_mut.search(body):
            return True
    return False


def immutable_lookup_type(inner):
    s = inner.strip()
    if s in (
        "()", "bool", "u8", "u16", "u32", "u64", "u128", "usize",
        "i8", "i16", "i32", "i64", "i128", "isize", "f32", "f64",
        "Instant", "Duration", "str", "&'static str", "String",
    ):
        return True
    if re.match(r"^&'static\s+", s):
        return True
    # Fixed-size arrays are treated as lookup tables (incl. Arc<Preset>).
    if re.match(r"^\[.*\s*;\s*[\w:]+\]$", s):
        return True
    if re.match(r"^Vec<(?:u\d+|i\d+|f\d+|usize|isize|bool)>$", s):
        return True
    if re.match(r"^(?:unsafe\s+)?(?:extern\s+\"[^\"]+\"\s+)?fn\s*\(", s):
        return True
    return False


def strip_init_cell_wrappers(ty):
    s = ty.strip()
    peeled = False
    for _ in range(4):
        m = re.match(
            r"^(?:std::sync::|once_cell::(?:sync|unsync)::|parking_lot::)?"
            r"(?:OnceLock|LazyLock|OnceCell|Lazy)\s*<\s*(.*)\s*>\s*$",
            s,
            re.S,
        )
        if not m:
            break
        s = m.group(1).strip()
        peeled = True
    return peeled, s


def init_cell_is_mutable(text, ty):
    if not init_cell.search(ty):
        return False
    peeled, inner = strip_init_cell_wrappers(ty)
    if not peeled:
        return False
    if immutable_lookup_type(inner):
        return False
    if re.match(r"^(?:std::sync::)?(?:Arc|Box|Mutex|RwLock|RefCell|Cell)\b", inner):
        return True
    if classic_mut.search(inner):
        return True
    if re.match(r"^(?:std::collections::)?(?:HashMap|BTreeMap)\s*<", inner):
        if "&'static" in inner:
            return False
        return True
    simple = re.match(r"^([A-Z][A-Za-z0-9_]*)\s*$", inner)
    if simple:
        name = simple.group(1)
        if name in ("Instant", "Duration"):
            return False
        # Same-file type alias to fn pointer / immutable (e.g. TimerFunc).
        alias = re.search(
            rf"(?m)^[ \t]*(?:pub(?:\([^)]*\))?[ \t]+)?type[ \t]+{re.escape(name)}[ \t]*=[ \t]*([^;]+);",
            text,
        )
        if alias and immutable_lookup_type(alias.group(1).strip()):
            return False
        body = struct_body_text(text, name)
        if body is None:
            return not struct_is_unit(text, name)
        return body_mut.search(body) is not None
    return True


def type_is_mutable(text, ty):
    if classic_mut.search(ty):
        return True
    # OnceLock/Lazy of a pure lookup table (arrays, fn pointers, …) must not be
    # counted just because a nested type name (e.g. VertexMaterialClass) has
    # interior mutability in its own definition.
    if init_cell.search(ty):
        peeled, inner = strip_init_cell_wrappers(ty)
        if peeled and immutable_lookup_type(inner):
            return False
        return init_cell_is_mutable(text, ty)
    if wrapper_struct_is_mutable(text, ty):
        return True
    return False


def cfg_test_depths(text):
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
            if text.startswith("mod ", j) or text.startswith("pub mod ", j):
                k = j
                while k < n and text[k] != "{":
                    if text[k] == ";":
                        break
                    k += 1
                if k < n and text[k] == "{":
                    test_depth = depth + 1
                    i = k
                    depth += 1
                    i += 1
                    continue
            else:
                depths.add(j)
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


def enclosing_thread_local(text, pos):
    """Return start index of thread_local! whose braces enclose pos, or -1."""
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
            return idx
        idx = text.rfind("thread_local!", 0, idx)
    return -1


def inside_cfg_test_thread_local(text, pos):
    idx = enclosing_thread_local(text, pos)
    if idx < 0:
        return False
    return item_has_cfg_test_attr(text, idx)


def inside_any_thread_local(text, pos):
    """Statics inside thread_local! are counted via the macro, not again."""
    return enclosing_thread_local(text, pos) >= 0


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

# Rust requires `#[global_allocator]` to be a `static`. If such a static is
# only the process allocator entry (stateless, or process-level alloc stats),
# list it here with the reason so the honest meter does not force contortions.
ALLOWED_STATIC_EXCEPTIONS = {
    # name: reason
    # Example: "GLOBAL_ALLOCATOR": "#[global_allocator] must be static (Rust)",
    # Source-counted even when the feature is off; bags only exist under
    # debug_crc / perf_timers and are process instrumentation, not game state.
    "CRC_DEBUG_BAG": "cfg(feature=debug_crc) instrumentation bag",
    "PERF_GATHER_REGISTRY": "cfg(feature=perf_timers) instrumentation bag",
}

# thread_local! ACTIVE_STACK for scoped Arc publish. Rust has no other way to
# bind a per-thread game-instance services bag without a process-wide static;
# the payload is Arc, not the game state itself.
ALLOWED_TLS_EXCEPTIONS = {
    # rel_path from GeneralsRust root: reason
    "Code/GameEngine/Common/src/common/common_services.rs":
        "CommonServices ACTIVE_STACK — scoped Arc bind for the game instance",
    "Code/Main/src/main_services.rs":
        "MainServices ACTIVE_STACK — scoped Arc bind for main/host services",
    "Code/GameEngine/GameLogic/src/system/engine_stores.rs":
        "WorldServices/EngineStores SCOPED_ACTIVE — scoped Arc bind for the game instance",
}


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
                    or (not with_cfg_test and item_has_cfg_test_attr(text, m.start()))
                ):
                    continue
                tls_key = f"{rel_dir}/{fn}"
                if tls_key in ALLOWED_TLS_EXCEPTIONS:
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
                    or type_is_mutable(text, ty)
                )
                if is_mut:
                    if (
                        m.start() in test_body
                        or (not with_cfg_test and item_has_cfg_test_attr(text, m.start()))
                        or (not with_cfg_test and inside_cfg_test_thread_local(text, m.start()))
                        or inside_any_thread_local(text, m.start())
                        or name in ALLOWED_STATIC_EXCEPTIONS
                    ):
                        continue
                    counts[label] += 1
                    details[label].append(f"{rel_dir}/{fn}:{name}")

if want_json:
    print(json.dumps(dict(counts), indent=2, sort_keys=True))
else:
    print(
        "Mutable process-wide statics ("
        + (
            "incl. #[cfg(test)] in prod files"
            if with_cfg_test
            else "excl. tests/examples/benches"
        )
        + "):"
    )
    total = 0
    for k, v in counts.items():
        print(f"  {k}: {v}")
        total += v
    print(f"  TOTAL: {total}")


# --- Opaque process-wrapper lint (always) ---
# Opaque *Slot/*Bag/*Runtime types hid interior Mutex/Atomic from older
# counters (renaming Bag→Runtime was enough to fake WWVegas=0). Existing
# wrappers are allowlisted; adding a new one fails this script — migrate
# onto owned per-instance context instead (see root AGENTS.md).
ALLOW_OPAQUE_WRAPPERS = {
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
    "AssetManagerBridgeRuntime",
    "AssetManagerSlot",
    "AsyncPlaybackHandlePoolSlot",
    "AudioEventLocalizationLanguageSlot",
    "AudioMainWindowHandleSlot",
    "AudioSettingsSlot",
    "AuthoredDrawWeaponBoneSlot",
    "BorrowedFxRuntime",
    "BoxDisplayMaskSlot",
    "BoxRenderRuntime",
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
    "CounterRuntime",
    "CounterSlot",
    "CrateSystemSlot",
    "CrateTestLockSlot",
    "CrcDebugBag",
    "CreditsManagerSlot",
    "CursorInfoMapSlot",
    "DamageFxListRuntime",
    "DamageFxStoreBag",
    "DamageFxStoreSlot",
    "DazzleManagerRuntime",
    "DazzleManagerSlotBag",
    "DazzleRenderingEnabledSlot",
    "DazzleRuntime",
    "DazzleTextureBag",
    "DazzleTypesBag",
    "DazzleVisibilityHandlerBag",
    "DebugCameraMoveLogsSlot",
    "DebugCameraMoveToLogsSlot",
    "DebugSystemSlot",
    "DecalIdCounterSlot",
    "DecalRuntime",
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
    "EngineRuntime",
    "EngineSlot",
    "EvaEventStoreSlot",
    "ExplicitVertexColorDebugsSlot",
    "ExtraBlockParsersSlot",
    "FilesSlot",
    "FontRuntime",
    "FormationSlot",
    "FrameActiveSlot",
    "FrameIndexAtSubmitSlot",
    "FrameSubmitBridgeRuntime",
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
    "GlobalManagerRuntime",
    "GlobalManagerSlot",
    "GlobalMemoryPoolFactorySlot",
    "GlobalRecorderSlot",
    "GlobalRendererRuntime",
    "GlobalRendererSlot",
    "GlobalScaleBitsSlot",
    "GuardRetaliateRuntime",
    "GuardRuntime",
    "HandlersRuntime",
    "HandlersSlot",
    "HeaderTemplateManagerSlot",
    "HookTestLockSlot",
    "HostBoneFxSlot",
    "HostDestroyLogLogBag",
    "HostFireIntentLogLogBag",
    "HostFxRuntime",
    "HostProductionProgressLogLogBag",
    "HostProjectileRuntime",
    "HostStateResidualTestEnvLockSlot",
    "HostTriggerSlot",
    "HotReloadRuntime",
    "HudFeedbackChainLockSlot",
    "HulkMaxLifetimeOverrideSlot",
    "IngameUiSettingsSlot",
    "InstanceRuntime",
    "InstanceSlot",
    "IsInittedSlot",
    "IsLockedSlot",
    "LanguageRuntime",
    "LastFrameSubmitCountSlot",
    "LastOutOfFrameCountSlot",
    "LauncherShutdownRequestedSlot",
    "LightingLodCutoffBag",
    "LiveGameClientSlot",
    "LoadScreenRuntime",
    "LockRuntime",
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
    "MissionScriptRuntime",
    "ModelManagerSlot",
    "ModelessDialogsSlot",
    "ModuleFactorySlot",
    "MouseCursorSlot",
    "MouseSettingsSlot",
    "MssLockDepthBag",
    "MultiplayerSettingsSlot",
    "NameKeyStateBag",
    "NativeDieRuntime",
    "NativeLifecycleRuntime",
    "NewestTimestampSlot",
    "NextHandleSlot",
    "NextIdSlot",
    "NextLivePopupGenerationSlot",
    "NextLogicalDecalIdSlot",
    "NextTempFileSlot",
    "ObjectDataProviderBag",
    "ObjectDefinitionRegistrySlot",
    "ObjectFxRuntime",
    "ObjectSlot",
    "OldestTimestampSlot",
    "OnlineChatColorsSlot",
    "PanicRuntime",
    "ParticleDebugDisableSlot",
    "ParticleEmitterRuntime",
    "ParticleSystemLiveOverlaySlot",
    "ParticleSystemManagerSlot",
    "ParticlesRuntime",
    "PendingSlot",
    "PerfGatherRegistryBag",
    "PlayerListLockSlot",
    "PlayerSlot",
    "PlayerTemplateStoreSlot",
    "PoolRegistrySlot",
    "PostLoadHooksSlot",
    "ProbeFrameCounterSlot",
    "ProductionConditionRuntime",
    "ProgramInstanceSlot",
    "QSlot",
    "RadarSystemSlot",
    "RankInfoStoreSlot",
    "RegistryByIdBag",
    "RegistryByIdRuntime",
    "RenderCallCountSlot",
    "Renderer3dRuntime",
    "RendererStorageSlot",
    "ReplaySlot",
    "ResolvedSkirmishSlot",
    "ResourceManagerSlot",
    "RetailGameDataProbeSlot",
    "RingIdCounterSlot",
    "RingManagerSlotBag",
    "RingMeshArrayBag",
    "RingRuntime",
    "RngLockSlot",
    "RuntimeIdCounterHooksSlot",
    "SaveLoadLifecycleHooksSlot",
    "SaveLoadManagerSlot",
    "SceneRuntime",
    "ScienceStoreSlot",
    "ScriptActionRuntime",
    "SettingsBag",
    "ShaderStateBag",
    "ShatterSystemBag",
    "ShatterSystemCsgRuntime",
    "ShatterSystemRuntime",
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
    "TestLockRuntime",
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
    "UnitAiRuntime",
    "UnitAiStateRuntime",
    "UpgradeCenterActiveSlot",
    "UpgradeCenterProcessLifetimeSlot",
    "VersionInstanceSlot",
    "VertexMaterialIdRuntime",
    "VideoPlayerSlot",
    "WarningCountRuntime",
    "WarningCountSlot",
    "WeaponStoreSlot",
    "WeatherSettingSlot",
    "WebBrowserSlot",
    "WgpuWrapperInstanceRuntime",
    "WgpuWrapperInstanceSlot",
    "WindowMsgPayloadSlot",
    "WindowTransitionStoreSlot",
    "WritableGlobalDataSlot",
    "Ww3DStateBag",
    "WwAudioInstanceBag",
    "WwAudioRuntime",
    "WwLibRuntime",
    "ZhRegistryBag"
}
pat_opaque_struct = re.compile(
    r"(?m)^[ \t]*(?:pub(?:\([^)]*\))?[ \t]+)?struct[ \t]+([A-Za-z_][A-Za-z0-9_]*(?:Slot|Bag|Runtime))\b"
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
            for m in pat_opaque_struct.finditer(body):
                name = m.group(1)
                if name not in ALLOW_OPAQUE_WRAPPERS:
                    # Ignore structs under #[cfg(test)] modules/attrs.
                    if m.start() in cfg_test_depths(body) or item_has_cfg_test_attr(body, m.start()):
                        continue
                    new_wrappers.append(f"{rel_dir}/{fn}:{name}")
if new_wrappers:
    print(
        "FAIL: new opaque Slot/Bag/Runtime wrappers "
        "(migrate to owned context, do not add):",
        file=sys.stderr,
    )
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
