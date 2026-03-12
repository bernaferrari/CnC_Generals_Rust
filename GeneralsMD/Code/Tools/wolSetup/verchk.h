#ifndef __VERCHK_H__
#define __VERCHK_H__

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>

// Obtain version information from the specified file.
bool GetVersionInfo(char* filename, VS_FIXEDFILEINFO* fileInfo);
bool loadWolapi( char *filename );

#endif // __VERCHK_H__

