// FILE: MiniLog.h /////////////////////////////////////////////////////////////
// Alternative logging
// Author: Matthew D. Campbell, January 2003
////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifdef DEBUG_LOGGING

#include "Lib/BaseType.h"
#include "GameLogic/GameLogic.h"
#include <cstdarg>
class LogClass
{
public:
	LogClass(const char *fname);
	~LogClass();
	void log(const char *fmt, ...);

protected:
	FILE *m_fp;
};

#endif // DEBUG_LOGGING
