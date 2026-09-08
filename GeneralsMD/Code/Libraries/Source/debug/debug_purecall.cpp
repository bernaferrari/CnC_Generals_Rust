/////////////////////////////////////////////////////////////////////////EA-V1
// $File: //depot/GeneralsMD/Staging/code/Libraries/Source/debug/debug_purecall.cpp $
// $Author: mhoffe $
// $Revision: #1 $
// $DateTime: 2003/07/03 11:55:26 $
//
// ©2003 Electronic Arts
//
// Replacement for MSVCRT _purecall
//////////////////////////////////////////////////////////////////////////////
#include "_pch.h"

// Pure virtual function called
int __cdecl _purecall(void)
{
  DCRASH_RELEASE("Pure virtual function called.");
  return 0;
}
