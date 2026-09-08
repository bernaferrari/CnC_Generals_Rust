// FILE: debug.cpp ////////////////////////////////////////////////////////
// Minmal debug info
// Author: Matthew D. Campbell, Sept 2002

#include <wdebug.h>
#include "debug.h"
#include <cstdio>

#ifdef DEBUG

#ifdef __cplusplus
extern "C" {
#endif

#ifdef _WINDOWS

#include <stdarg.h>
void DebugCrash( const char *fmt, ... ) {}
char* TheCurrentIgnoreCrashPtr;

#else

#endif

void DebugLog(const char *fmt, ...)
{
	static char buffer[1024];
	va_list va;
	va_start( va, fmt );
	vsnprintf(buffer, 1024, fmt, va );
	buffer[1023] = 0;
	va_end( va );

	//printf( buffer );
	DBGMSG(buffer);
}

#ifdef __cplusplus
}
#endif

#endif // DEBUG

