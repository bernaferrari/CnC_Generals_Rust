// Used for dynamically linking to dbghelp.dll functions.

// keep this always as first entry
DBGHELP(SymInitialize,
        BOOL,
        (HANDLE hProcess, PCSTR UserSearchPath, BOOL fInvadeProcess))

DBGHELP(SymGetOptions,
        DWORD,
        (void))

DBGHELP(SymSetOptions,
        DWORD,
        (DWORD SymOptions))

DBGHELP(StackWalk,
        BOOL,
        (DWORD MachineType, HANDLE hProcess, HANDLE hThread, LPSTACKFRAME StackFrame, 
        LPVOID ContextRecord, PREAD_PROCESS_MEMORY_ROUTINE ReadMemoryRoutine, 
        PFUNCTION_TABLE_ACCESS_ROUTINE FunctionTableAccessRoutine, 
        PGET_MODULE_BASE_ROUTINE GetModuleBaseRoutine, 
        PTRANSLATE_ADDRESS_ROUTINE TranslateAddress))

DBGHELP(SymFunctionTableAccess,
        LPVOID,
        (HANDLE hProcess, DWORD AddrBase))

DBGHELP(SymGetModuleBase,
        DWORD,
        (HANDLE hProcess, DWORD dwAddr))

DBGHELP(SymGetSymFromAddr,
        BOOL,
        (HANDLE hProcess, DWORD Address, LPDWORD Displacement, 
        PIMAGEHLP_SYMBOL Symbol))

DBGHELP(SymGetLineFromAddr,
        BOOL,
        (HANDLE hProcess, DWORD dwAddr, PDWORD pdwDisplacement, 
        PIMAGEHLP_LINE Line))

// keep this always as last entry
DBGHELP(SymCleanup,
        BOOL,
        (HANDLE hProcess)) 
