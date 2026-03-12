// FILE: MiniLog.cpp ///////////////////////////////////////////////////////////
// Alternative logging
// Author: Matthew D. Campbell, January 2003
////////////////////////////////////////////////////////////////////////////////

#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine
#include "Common/MiniLog.h"

#ifdef DEBUG_LOGGING

LogClass::LogClass(const char *fname)
{
	char buffer[ _MAX_PATH ];
	GetModuleFileName( NULL, buffer, sizeof( buffer ) );
	char *pEnd = buffer + strlen( buffer );
	while( pEnd != buffer )
	{
		if( *pEnd == '\\' )
		{
			*pEnd = 0;
			break;
		}
		pEnd--;
	}
	AsciiString fullPath;
	fullPath.format("%s\\%s", buffer, fname);
	m_fp = fopen(fullPath.str(), "wt");
}

LogClass::~LogClass()
{
	if (m_fp)
	{
		fclose(m_fp);
	}
}

void LogClass::log(const char *fmt, ...)
{
	if (!m_fp)
		return;
	static char buf[1024];
	static Int lastFrame = 0;
	static Int lastIndex = 0;
	if (lastFrame != TheGameLogic->getFrame())
	{
		lastFrame = TheGameLogic->getFrame();
		lastIndex = 0;
	}

	va_list va;
	va_start( va, fmt );
	_vsnprintf(buf, 1024, fmt, va );
	buf[1023] = 0;
	va_end( va );

	char *tmp = buf;
	while (tmp && *tmp)
	{
		if (*tmp == '\r' || *tmp == '\n')
		{
			*tmp = ' ';
		}
		++tmp;
	}

	fprintf(m_fp, "%d:%d %s\n", lastFrame, lastIndex++, buf);
	fflush(m_fp);
}

#endif // DEBUG_LOGGING
