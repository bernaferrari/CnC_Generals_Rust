// DownloadDebug.h

#ifndef __DOWNLOADDEBUG_H_
#define __DOWNLOADDEBUG_H_

#ifdef  NDEBUG
#define DEBUG_LOG(exp) ((void)0)
#else

#ifdef __cplusplus
extern "C" {
#endif

	#include <stdarg.h>
	extern void DebugCrash( const char *fmt, ... );
	extern void DebugLog( const char *fmt, ... );

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

	#define DEBUG_LOG(x)		do { DebugLog x; } while (0)
	#define DEBUG_ASSERTCRASH(c, m)		do { { if (!(c)) DEBUG_CRASH(m); } } while (0)

#ifdef __cplusplus
}
#endif

#endif // NDEBUG

#endif //__DOWNLOADDEBUG_H_
