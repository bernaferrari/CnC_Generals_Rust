// FILE: debug.h //////////////////////////////////////////////////////////////
// Minimal debug info
// Author: Matthew D. Campbell, Sept 2002

#ifndef __DEBUG_H__
#define __DEBUG_H__

#ifdef DEBUG

#include <cstdarg>

void DebugLog( const char *fmt, ... );
#define DEBUG_LOG(x) DebugLog x

#else // DEBUG

#define DEBUG_LOG(x) {}

#endif // DEBUG


#ifdef DEBUG_CRASHING

	extern void DebugCrash(const char *format, ...);

	/*
		Yeah, it's a sleazy global, since we can't reasonably add
		any args to DebugCrash due to the varargs nature of it. 
		We'll just let it slide in this case...
	*/
	extern char* TheCurrentIgnoreCrashPtr;

	#define DEBUG_CRASH(m)	\
		do { \
			{ \
				static char ignoreCrash = 0; \
				if (!ignoreCrash) { \
					TheCurrentIgnoreCrashPtr = &ignoreCrash; \
					DebugCrash m ; \
					TheCurrentIgnoreCrashPtr = NULL; \
				} \
			} \
		} while (0)

	#define DEBUG_ASSERTCRASH(c, m)		do { { if (!(c)) DEBUG_CRASH(m); } } while (0)

#else

	#define DEBUG_CRASH(m)					((void)0)
	#define DEBUG_ASSERTCRASH(c, m)	((void)0)

#endif

#endif // __DEBUG_H__

