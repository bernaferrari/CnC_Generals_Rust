// FILE: debug.cpp ////////////////////////////////////////////////////////
// Minmal debug info
// Author: Matthew D. Campbell, Sept 2002

#include <windows.h>
#include "debug.h"
#include <cstdio>

#ifdef DEBUG

void DebugLog(const char *fmt, ...)
{
	static char buffer[1024];
	va_list va;
	va_start( va, fmt );
	vsnprintf(buffer, 1024, fmt, va );
	buffer[1023] = 0;
	va_end( va );

	printf( "%s", buffer );
	OutputDebugString( buffer );
}

#endif // DEBUG

