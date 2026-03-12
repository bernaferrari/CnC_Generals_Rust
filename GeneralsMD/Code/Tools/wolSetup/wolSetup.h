// FILE: wolSetup.h //////////////////////////////////////////////////////
// Author: Matthew D. Campbell, December 2001

#ifndef __WOLSETUP_H__
#define __WOLSETUP_H__

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>

void checkInstalledWolapiVersion( void );
void setupGenerals( const char *genPath, const char *genSerial );

extern HINSTANCE g_hInst;
extern unsigned long g_wolapiRegistryVersion;
extern unsigned long g_wolapiRealVersion;
extern bool g_wolapiInstalled;
extern char g_wolapiRegFilename[MAX_PATH];
extern char g_wolapiRealFilename[MAX_PATH];
extern char g_generalsFilename[MAX_PATH];
extern char g_generalsSerial[];

static MAJOR(unsigned long x) { return (((x) & 0xffff0000) >> 16); }
static MINOR(unsigned long x) { return ((x) & 0xffff); }

#endif // __WOLSETUP_H__
