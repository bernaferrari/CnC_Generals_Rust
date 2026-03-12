#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine
#include "Common/CriticalSection.h"

// Definitions.
FastCriticalSectionClass TheAsciiStringCriticalSection;
CriticalSection *TheUnicodeStringCriticalSection = NULL;
CriticalSection *TheDmaCriticalSection = NULL;
CriticalSection *TheMemoryPoolCriticalSection = NULL;
CriticalSection *TheDebugLogCriticalSection = NULL;

#ifdef PERF_TIMERS
PerfGather TheCritSecPerfGather("CritSec");
#endif

