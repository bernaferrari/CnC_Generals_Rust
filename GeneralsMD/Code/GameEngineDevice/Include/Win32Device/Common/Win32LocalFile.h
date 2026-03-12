///// Win32LocalFile.h ///////////////////////////
// Bryan Cleveland, August 2002
//////////////////////////////////////////////////

#pragma once

#ifndef __WIN32LOCALFILE_H
#define __WIN32LOCALFILE_H

#include "Common/LocalFile.h"

class Win32LocalFile : public LocalFile
{
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE(Win32LocalFile, "Win32LocalFile")		
public:
	Win32LocalFile();
	//virtual ~Win32LocalFile();

protected:
};

#endif // __WIN32LOCALFILE_H