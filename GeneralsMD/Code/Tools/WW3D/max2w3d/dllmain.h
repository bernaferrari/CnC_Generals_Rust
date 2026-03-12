#ifndef DLLMAIN_H
#define DLLMAIN_H

#include <windows.h>

extern HINSTANCE AppInstance;

#define MAX_STRING_LENGTH 256
#define MAX_PATH_LENGTH _MAX_PATH

TCHAR * Get_String(int id);

#endif /*DLLMAIN_H*/