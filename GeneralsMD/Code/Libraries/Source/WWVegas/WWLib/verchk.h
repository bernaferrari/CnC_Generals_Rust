#if defined(_MSC_VER)
#pragma once
#endif

#ifndef __VERCHK_H
#define __VERCHK_H

#include <windows.h>

// Obtain version information from the specified file.
bool GetVersionInfo(char* filename, VS_FIXEDFILEINFO* fileInfo);

// Retreive creation time of specified file.
bool GetFileCreationTime(char* filename, FILETIME* createTime);

////////////////////////////////////////////////////////////////////////
//
//	Compare_EXE_Version
//
//	Used to compare 2 versions of an executable, the currently executing
// exe and a version saved to disk.  These exe's do not need to have
// a version resource.
//
//	The return value is the same as strcmp, -1 if the current process is
// older, 0 if they are the same, and +1 if the current process is newer.
//
////////////////////////////////////////////////////////////////////////
int Compare_EXE_Version (int app_instance, const char *filename);


#endif //__VERCHK_H

