/******************************************************************************
*
* FILE
*     $Archive:  $
*
* DESCRIPTION
*     Debug printing mechanism
*
* PROGRAMMER
*     Denzil E. Long, Jr.
*     $Author:  $
*
* VERSION INFO
*     $Modtime:  $
*     $Revision:  $
*
******************************************************************************/

#ifndef _DEBUGPRINT_H_
#define _DEBUGPRINT_H_

#ifdef _DEBUG

#ifdef __cplusplus
extern "C"
{
#endif

//! Ouput debug print messages to the debugger and log file.
void __cdecl DebugPrint(const char* string, ...);
void __cdecl PrintWin32Error(const char* string, ...);

extern char debugLogName[];


#ifdef __cplusplus
}
#endif

#else // _DEBUG

#define DebugPrint
#define PrintWin32Error

#endif // _DEBUG

#endif // _DEBUGPRINT_H_
