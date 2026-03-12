#include "always.h"
#include "win.h"
#include "wwdebug.h"

HINSTANCE	ProgramInstance;
HWND			MainWindow;
bool GameInFocus = false;

/***********************************************************************************************
 * Print_Win32Error -- Print the Win32 error message.                                          *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   6/21/01    DEL : Created.                                                                 *
 *=============================================================================================*/
#ifdef _DEBUG
void __cdecl Print_Win32Error(unsigned long win32Error)
{
	LPVOID lpMsgBuf;
	FormatMessage(FORMAT_MESSAGE_ALLOCATE_BUFFER | FORMAT_MESSAGE_FROM_SYSTEM |
			FORMAT_MESSAGE_IGNORE_INSERTS, NULL, win32Error, MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT),
			(LPTSTR)&lpMsgBuf, 0, NULL);

	WWDEBUG_SAY(("Win32 Error: %s\n", (const char*)lpMsgBuf));
	LocalFree(lpMsgBuf);
}
#endif

